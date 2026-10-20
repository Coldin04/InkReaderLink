//! `Pico Page` browser-uploader protocol adapter.

use std::path::{Path, PathBuf};

use tokio::io::AsyncReadExt;
use url::Url;

use crate::{
    SdkError,
    transport::{HttpBody, HttpMethod, HttpRequest, HttpResponse},
};

pub const ACCESS_CODE_KEY: &str = "access_code";
const INVALID_ADDRESS_PROMPT: &str = "请用手机扫描设备屏幕上的二维码，或在浏览器输入屏幕上显示的完整地址（包括斜杠后的字母和数字）。";
const CRC32_TABLE: [u32; 256] = crc32_table();

#[derive(Debug, Default)]
pub struct PicoPageAdapter;

impl PicoPageAdapter {
    /// Splits a Pico Page URL into its HTTP origin and optional route access code.
    ///
    /// A bare origin is accepted so the client can request the code through its
    /// normal session-credential flow.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported schemes, malformed URLs, or invalid route paths.
    pub fn connection_origin(address: &str) -> Result<(String, Option<String>), SdkError> {
        let normalized = if address.contains("://") {
            address.to_owned()
        } else {
            format!("http://{address}")
        };
        let mut url = Url::parse(&normalized)
            .map_err(|error| SdkError::InvalidArgument(format!("invalid device URL: {error}")))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(SdkError::InvalidArgument(
                "device URL must use HTTP or HTTPS and contain a host".to_owned(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(SdkError::InvalidArgument(
                "device URL must not contain user information".to_owned(),
            ));
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(SdkError::InvalidArgument(
                "Pico Page address must not contain a query or fragment".to_owned(),
            ));
        }

        let raw_path = url.path();
        let path = raw_path.strip_prefix('/').unwrap_or(raw_path);
        let path = path.strip_suffix('/').unwrap_or(path);
        let access_code = if raw_path == "/" {
            None
        } else if !path.is_empty()
            && !path.contains('/')
            && path.len() <= 64
            && path.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            Some(path.to_owned())
        } else {
            return Err(SdkError::InvalidArgument(INVALID_ADDRESS_PROMPT.to_owned()));
        };

        url.set_path("/");
        Ok((url.origin().ascii_serialization(), access_code))
    }

    /// Builds a request for the route page carrying the access code.
    ///
    /// # Errors
    ///
    /// Returns an error when the access code contains characters outside ASCII
    /// letters and digits.
    pub fn page_request(access_code: &str) -> Result<HttpRequest, SdkError> {
        validate_access_code(access_code)?;
        let mut request = HttpRequest::new(HttpMethod::Get, format!("/{access_code}"));
        request.response_limit = Some(2 * 1024 * 1024);
        request.sensitive = true;
        Ok(request)
    }

    /// Validates the page reached by the route code and identifies an invalid code.
    ///
    /// # Errors
    ///
    /// Returns `InvalidArgument` for a malformed code, `CredentialRejected` for an
    /// invalid code, or `RemoteFailure` when the response is not a Pico Page uploader.
    pub fn validate_page(response: &HttpResponse, access_code: &str) -> Result<String, SdkError> {
        validate_access_code(access_code)?;
        if response.status == 401 || response.status == 403 || response.status == 404 {
            return Err(SdkError::CredentialRejected {
                key: ACCESS_CODE_KEY.to_owned(),
            });
        }

        let body = response.text_lossy();
        if body.contains("请用手机扫描设备屏幕上的二维码")
            && body.contains("完整地址")
            && body.contains("斜杠后的字母和数字")
        {
            return Err(SdkError::CredentialRejected {
                key: ACCESS_CODE_KEY.to_owned(),
            });
        }
        if response.status != 200 {
            return Err(SdkError::RemoteFailure(format!(
                "verify Pico Page address failed with HTTP {}: {}",
                response.status,
                body.trim()
            )));
        }
        let lower = body.to_ascii_lowercase();
        if !lower.contains("id=\"files\"") || !lower.contains("id=\"list\"") {
            return Err(SdkError::RemoteFailure(
                "address does not serve a Pico Page uploader".to_owned(),
            ));
        }
        extract_upload_token(&body).ok_or_else(|| {
            SdkError::RemoteFailure("Pico Page uploader page has no valid session token".to_owned())
        })
    }

    #[must_use]
    pub fn check_request(
        file_name: &str,
        size: u64,
        crc32: &str,
        session_token: &str,
        origin: Option<&str>,
        access_code: &str,
    ) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Post, "/check");
        Self::add_upload_headers(
            &mut request,
            file_name,
            size,
            crc32,
            session_token,
            origin,
            access_code,
        );
        // The browser uploader's `xhr.send(null)` emits an explicit zero-length
        // request body. Keep the same framing for Pico Page's preflight request.
        request
            .headers
            .push(("Content-Length".to_owned(), "0".to_owned()));
        request.response_limit = Some(1024);
        request
    }

    #[must_use]
    pub fn upload_request(
        local_path: PathBuf,
        file_name: &str,
        size: u64,
        crc32: &str,
        session_token: &str,
        origin: Option<&str>,
        access_code: &str,
    ) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Post, "/upload");
        Self::add_upload_headers(
            &mut request,
            file_name,
            size,
            crc32,
            session_token,
            origin,
            access_code,
        );
        request.body = HttpBody::RawFile {
            path: local_path,
            content_type: None,
        };
        request.response_limit = Some(1024);
        request
    }

    fn add_upload_headers(
        request: &mut HttpRequest,
        file_name: &str,
        size: u64,
        crc32: &str,
        session_token: &str,
        origin: Option<&str>,
        access_code: &str,
    ) {
        request.sensitive = true;
        request.headers.extend([
            ("X-Pico-Token".to_owned(), session_token.to_owned()),
            ("X-Pico-Name".to_owned(), encode_uri_component(file_name)),
            ("X-Pico-CRC".to_owned(), crc32.to_owned()),
            ("X-Pico-Size".to_owned(), size.to_string()),
        ]);
        if let Some(origin) = origin {
            request
                .headers
                .push(("Origin".to_owned(), origin.to_owned()));
            request.headers.push((
                "Referer".to_owned(),
                format!("{}/{access_code}", origin.trim_end_matches('/')),
            ));
        }
    }

    /// Calculates the firmware's CRC-32 over a file using bounded streaming I/O.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read.
    pub async fn file_checksum(path: &Path) -> Result<(u64, String), SdkError> {
        let mut file = tokio::fs::File::open(path).await.map_err(|error| {
            SdkError::InvalidArgument(format!("cannot read upload file: {error}"))
        })?;
        let size = file
            .metadata()
            .await
            .map_err(|error| {
                SdkError::InvalidArgument(format!("cannot read upload file: {error}"))
            })?
            .len();
        let mut crc = 0xffff_ffffu32;
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).await.map_err(|error| {
                SdkError::InvalidArgument(format!("cannot read upload file: {error}"))
            })?;
            if count == 0 {
                break;
            }
            for byte in &buffer[..count] {
                crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ u32::from(*byte)) & 0xff) as usize];
            }
        }
        let checksum = crc ^ 0xffff_ffff;
        Ok((size, format!("{checksum:x}")))
    }

    /// Returns the firmware's upload error text without assigning an SDK category.
    ///
    /// # Errors
    ///
    /// Returns the error represented by any response status other than HTTP 200.
    pub fn ensure_success(response: &HttpResponse, operation: &str) -> Result<(), SdkError> {
        if response.status == 200 {
            return Ok(());
        }
        let message = response.text_lossy();
        let message = if message.trim().is_empty() {
            "设备拒绝了这个文件。".to_owned()
        } else {
            message
        };
        Err(SdkError::RemoteFailure(format!(
            "{operation} failed with HTTP {}: {message}",
            response.status
        )))
    }
}

fn extract_upload_token(body: &str) -> Option<String> {
    let (_, assignment) = body.split_once("const token")?;
    let assignment = assignment.trim_start().strip_prefix('=')?.trim_start();
    let quote = assignment.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let value = assignment.get(quote.len_utf8()..)?;
    let end = value.find(quote)?;
    let token = &value[..end];
    if token.is_empty()
        || token.len() > 128
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return None;
    }
    Some(token.to_owned())
}

fn encode_uri_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn validate_access_code(access_code: &str) -> Result<(), SdkError> {
    if access_code.is_empty()
        || access_code.len() > 64
        || !access_code.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(SdkError::InvalidArgument(INVALID_ADDRESS_PROMPT.to_owned()));
    }
    Ok(())
}

const fn crc32_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut index = 0;
    let mut seed = 0_u32;
    while index < table.len() {
        let mut value = seed;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ if value & 1 == 1 { 0xedb8_8320 } else { 0 };
            bit += 1;
        }
        table[index] = value;
        index += 1;
        seed += 1;
    }
    table
}

pub(crate) mod session;

pub(crate) mod definition;

#[cfg(test)]
mod session_tests;
