//! `PageRest` API adapter. Its book listing, Wi-Fi policy, per-kind preflight,
//! media roots, and upload reconciliation use PageRest's own wire contract.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    DeviceInfoField, FileEntry, FileKind, FileLocation, SdkError, WifiNetwork,
    transport::{HttpBody, HttpMethod, HttpRequest, HttpResponse},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageRestUploadKind {
    Book,
    Font,
    Image,
}

impl PageRestUploadKind {
    const fn as_query_value(self) -> &'static str {
        match self {
            Self::Book => "book",
            Self::Font => "font",
            Self::Image => "image",
        }
    }

    const fn upload_path(self) -> &'static str {
        match self {
            Self::Book => "/upload",
            Self::Font => "/fonts",
            Self::Image => "/images",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
// These booleans are independent fields in the firmware wire schema.
#[allow(clippy::struct_excessive_bools)]
pub struct PageRestInfo {
    pub is_flash: bool,
    pub free_bytes: u64,
    #[serde(default)]
    pub file_limit: Option<u64>,
    pub mode: String,
    pub wifi_configured: bool,
    #[serde(default)]
    pub wifi_ssid: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub fonts_enabled: bool,
    #[serde(default)]
    pub font_limit: Option<u64>,
    #[serde(default)]
    pub font_root: Option<String>,
    #[serde(default)]
    pub images_enabled: bool,
    #[serde(default)]
    pub image_limit: Option<u64>,
    #[serde(default)]
    pub image_root: Option<String>,
    #[serde(default)]
    pub upload_seq: Option<u64>,
    #[serde(default)]
    pub last_upload: Option<PageRestLastUpload>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PageRestLastUpload {
    pub seq: u64,
    pub name: String,
    pub total: u64,
    #[serde(default)]
    pub received: u64,
    pub status: u16,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct UploadCheck {
    exists: bool,
    #[serde(default)]
    upload_seq: Option<u64>,
}

#[derive(Deserialize)]
struct PageRestBookPage {
    page: usize,
    total: usize,
    pages: usize,
    items: Vec<PageRestBook>,
}

#[derive(Deserialize)]
struct PageRestBook {
    name: String,
    size: u64,
}

#[derive(Deserialize)]
struct PageRestWifiInfo {
    wifi_configured: bool,
    #[serde(default)]
    wifi_ssid: Option<String>,
}

#[derive(Serialize)]
struct PageRestWifiCredential<'a> {
    ssid: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct FileSystemResponse {
    entries: Vec<FileSystemEntry>,
}

#[derive(Deserialize)]
struct FileSystemEntry {
    name: String,
    size: u64,
    #[serde(default)]
    dir: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct UploadCheckResult {
    pub exists: bool,
    pub upload_seq: Option<u64>,
}

#[derive(Debug, Default)]
pub struct PageRestAdapter;

impl PageRestAdapter {
    #[must_use]
    pub fn book_list_request(page: usize) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/books");
        request.query.push(("page".to_owned(), page.to_string()));
        request
    }

    /// # Errors
    /// Returns a remote failure for malformed book listings or unsupported locations.
    pub fn parse_book_page(
        location: &crate::FileLocation,
        body: &str,
    ) -> Result<super::AdapterFilePage, SdkError> {
        if !matches!(location, FileLocation::Root) {
            return Err(SdkError::Unsupported(
                "PageRest does not support directory selection".to_owned(),
            ));
        }
        let response: PageRestBookPage = serde_json::from_str(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid PageRest book list: {error}"))
        })?;
        let entries = response
            .items
            .into_iter()
            .map(|book| FileEntry {
                path: format!("/{}", book.name),
                name: book.name,
                size: book.size,
                kind: FileKind::Book,
            })
            .collect();
        Ok(super::AdapterFilePage {
            entries,
            page: response.page,
            total: response.total,
            pages: response.pages,
        })
    }

    pub(super) fn book_name_from_path(path: &str) -> Result<&str, SdkError> {
        let name = path.strip_prefix('/').unwrap_or(path);
        crate::validation::validate_file_name(name)?;
        Ok(name)
    }

    #[must_use]
    pub fn info_request() -> HttpRequest {
        HttpRequest::new(HttpMethod::Get, "/info")
    }

    /// Validates `PageRest`'s distinguishing `/info` fields and returns its policy snapshot.
    /// # Errors
    /// Returns a remote failure when the device policy fields are missing or malformed.
    pub fn parse_info(body: &[u8]) -> Result<PageRestInfo, SdkError> {
        serde_json::from_slice(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid PageRest device info: {error}"))
        })
    }

    /// # Errors
    /// Returns a remote failure when the distinguishing device fields are invalid.
    pub fn validate_info(body: &[u8]) -> Result<(), SdkError> {
        Self::parse_info(body).map(|_| ())
    }

    /// # Errors
    /// Returns a remote failure when the device information cannot be parsed.
    pub fn device_info(body: &[u8]) -> Result<Vec<DeviceInfoField>, SdkError> {
        let info = Self::parse_info(body)?;
        let mut fields = vec![
            field("storage_is_flash", info.is_flash),
            field("storage_free_bytes", info.free_bytes),
            field("network_mode", info.mode),
            field("wifi_configured", info.wifi_configured),
            field("fonts_enabled", info.fonts_enabled),
            field("images_enabled", info.images_enabled),
        ];
        if let Some(root) = info.root {
            fields.push(field("storage_root", root));
        }
        if let Some(root) = info.font_root {
            fields.push(field("font_root", root));
        }
        if let Some(root) = info.image_root {
            fields.push(field("image_root", root));
        }
        if let Some(limit) = info.file_limit {
            fields.push(field("storage_file_limit", limit));
        }
        if let Some(ssid) = info.wifi_ssid.filter(|ssid| !ssid.is_empty()) {
            fields.push(field("wifi_ssid", ssid));
        }
        if let Some(limit) = info.font_limit {
            fields.push(field("font_file_limit", limit));
        }
        if let Some(limit) = info.image_limit {
            fields.push(field("image_file_limit", limit));
        }
        Ok(fields)
    }

    #[must_use]
    pub fn upload_check_request(
        kind: PageRestUploadKind,
        file_name: &str,
        size: u64,
    ) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Post, "/upload-check");
        request.query.extend([
            ("kind".to_owned(), kind.as_query_value().to_owned()),
            ("name".to_owned(), file_name.to_owned()),
            ("size".to_owned(), size.to_string()),
        ]);
        request
            .headers
            .push(("Content-Length".to_owned(), "0".to_owned()));
        request
    }

    /// # Errors
    /// Returns a remote failure for unsuccessful or malformed preflight responses.
    pub fn parse_upload_check(response: &HttpResponse) -> Result<UploadCheckResult, SdkError> {
        Self::ensure_success(response, "check upload")?;
        let check: UploadCheck = serde_json::from_slice(&response.body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid PageRest upload check response: {error}"))
        })?;
        Ok(UploadCheckResult {
            exists: check.exists,
            upload_seq: check.upload_seq,
        })
    }

    #[must_use]
    pub fn upload_request(
        kind: PageRestUploadKind,
        path: PathBuf,
        file_name: &str,
        overwrite: bool,
        content_type: Option<String>,
    ) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Put, kind.upload_path());
        request
            .query
            .push(("name".to_owned(), file_name.to_owned()));
        if overwrite {
            request.query.push(("overwrite".to_owned(), "1".to_owned()));
        }
        request.body = HttpBody::RawFile {
            path,
            content_type: content_type
                .or_else(|| page_rest_content_type(file_name).map(str::to_owned)),
        };
        request
    }

    #[must_use]
    pub fn files_request(root: &str) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/fs");
        request.query.push(("root".to_owned(), root.to_owned()));
        request
    }

    /// # Errors
    /// Returns a remote failure for malformed entries or unsafe file paths.
    pub fn parse_media_files(body: &[u8], root: &str) -> Result<Vec<FileEntry>, SdkError> {
        let response: FileSystemResponse = serde_json::from_slice(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid PageRest {root} file list: {error}"))
        })?;
        Ok(response
            .entries
            .into_iter()
            .filter(|entry| !entry.dir)
            .map(|entry| FileEntry {
                path: format!("/{root}/{}", entry.name),
                name: entry.name,
                size: entry.size,
                kind: FileKind::Other,
            })
            .collect())
    }

    #[must_use]
    pub fn delete_request(kind: PageRestUploadKind, file_name: &str) -> HttpRequest {
        let path = match kind {
            PageRestUploadKind::Book => "/books",
            PageRestUploadKind::Font => "/fonts",
            PageRestUploadKind::Image => "/images",
        };
        let mut request = HttpRequest::new(HttpMethod::Delete, path);
        request
            .query
            .push(("name".to_owned(), file_name.to_owned()));
        request
    }

    /// # Errors
    /// Returns a remote failure when the saved network response cannot be parsed.
    pub fn parse_wifi_info(body: &str) -> Result<Vec<crate::WifiNetwork>, SdkError> {
        let info: PageRestWifiInfo = serde_json::from_str(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid PageRest Wi-Fi info: {error}"))
        })?;
        Ok(info
            .wifi_ssid
            .filter(|ssid| info.wifi_configured && !ssid.is_empty())
            .into_iter()
            .map(|ssid| WifiNetwork {
                index: None,
                ssid,
                has_password: info.wifi_configured,
                is_last_connected: true,
            })
            .collect())
    }

    /// # Errors
    /// Returns an argument error if the Wi-Fi credentials cannot be serialized.
    pub fn wifi_save_request(credential: &crate::WifiCredential) -> Result<HttpRequest, SdkError> {
        let body = serde_json::to_vec(&PageRestWifiCredential {
            ssid: &credential.ssid,
            password: credential.password.as_deref().unwrap_or_default(),
        })
        .map_err(|error| SdkError::InvalidArgument(format!("invalid Wi-Fi data: {error}")))?;
        let mut request = HttpRequest::new(HttpMethod::Post, "/wifi");
        request.body = HttpBody::Json(body);
        Ok(request)
    }

    #[must_use]
    pub fn wifi_delete_request() -> HttpRequest {
        HttpRequest::new(HttpMethod::Delete, "/wifi")
    }

    /// # Errors
    /// Returns a remote failure for any status other than HTTP 200.
    pub fn ensure_success(response: &HttpResponse, operation: &str) -> Result<(), SdkError> {
        if response.status == 200 {
            return Ok(());
        }
        Err(SdkError::RemoteFailure(format!(
            "{operation} failed with HTTP {}: {}",
            response.status,
            response_message(response)
        )))
    }

    /// # Errors
    /// Returns a conflict, remote failure, or committed-operation warning from the upload response.
    pub fn ensure_upload_success(response: &HttpResponse) -> Result<(), SdkError> {
        if response.status == 409 {
            return Err(SdkError::Conflict(response_message(response)));
        }
        Self::ensure_mutation_success(response, "upload")
    }

    /// # Errors
    /// Returns a remote failure, cleanup failure, or committed-operation warning from the response.
    pub fn ensure_mutation_success(
        response: &HttpResponse,
        operation: &str,
    ) -> Result<(), SdkError> {
        if response.status != 200 {
            return Self::ensure_success(response, operation);
        }
        let body: serde_json::Value = serde_json::from_slice(&response.body).map_err(|error| {
            SdkError::RemoteFailure(format!(
                "{operation} response could not be parsed; check whether the change was saved: {error}"
            ))
        })?;
        if body
            .get("progress_cleanup_failed")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            let detail = body
                .get("error")
                .and_then(serde_json::Value::as_str)
                .filter(|message| !message.trim().is_empty())
                .unwrap_or("reading progress cleanup failed");
            let warning = body
                .get("warning")
                .and_then(serde_json::Value::as_str)
                .filter(|message| !message.trim().is_empty());
            let message = warning.map_or_else(
                || format!("{operation} completed, but {detail}"),
                |warning| format!("{operation} completed, but {detail}; {warning}"),
            );
            return Err(SdkError::CommittedButCleanupFailed(message));
        }
        if let Some(warning) = body
            .get("warning")
            .and_then(serde_json::Value::as_str)
            .filter(|message| !message.trim().is_empty())
        {
            return Err(SdkError::CommittedWithWarning(warning.to_owned()));
        }
        Ok(())
    }

    /// Mirrors `PageRest`'s browser recovery: a failed transfer is attributed to the
    /// device only when its upload sequence, name, and total match this request.
    #[must_use]
    pub fn matching_upload_failure(
        info: &PageRestInfo,
        previous_sequence: Option<u64>,
        file_name: &str,
        size: u64,
    ) -> Option<String> {
        let upload = info.last_upload.as_ref()?;
        if previous_sequence.is_none_or(|sequence| sequence == upload.seq)
            || upload.name != file_name
            || upload.total != size
            || upload.status < 400
        {
            return None;
        }
        let message = upload
            .error
            .as_deref()
            .filter(|message| !message.is_empty())?;
        Some(format!(
            "{message}；设备收到 {} / {} 字节",
            upload.received, size
        ))
    }
}

fn response_message(response: &HttpResponse) -> String {
    serde_json::from_slice::<serde_json::Value>(&response.body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| {
            let body = response.text_lossy();
            if body.trim().is_empty() {
                "device rejected the request".to_owned()
            } else {
                body
            }
        })
}

fn page_rest_content_type(file_name: &str) -> Option<&'static str> {
    let extension = std::path::Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())?;
    if extension.eq_ignore_ascii_case("epub") {
        Some("application/epub+zip")
    } else if extension.eq_ignore_ascii_case("txt") {
        Some("text/plain")
    } else if extension.eq_ignore_ascii_case("ttf") {
        Some("font/ttf")
    } else if extension.eq_ignore_ascii_case("png") {
        Some("image/png")
    } else if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
        Some("image/jpeg")
    } else {
        None
    }
}

#[allow(clippy::needless_pass_by_value)]
fn field(key: &str, value: impl ToString) -> DeviceInfoField {
    DeviceInfoField {
        key: key.to_owned(),
        value: value.to_string(),
    }
}

pub(crate) mod session;

pub(crate) mod definition;

#[cfg(test)]
mod session_tests;
