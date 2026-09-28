//! `Read Pico` protocol adapter.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    FileEntry, FileKind, FileLocation, SdkError, WifiCredential, WifiNetwork,
    transport::{HttpBody, HttpMethod, HttpRequest},
};

use super::AdapterFilePage;

/// `Read Pico` protocol implementation.
#[derive(Debug, Default)]
pub struct ReadPicoAdapter;

#[derive(Deserialize)]
struct ReadPicoBook {
    name: String,
    size: u64,
}

#[derive(Deserialize)]
struct ReadPicoBookPage {
    page: usize,
    total: usize,
    pages: usize,
    items: Vec<ReadPicoBook>,
}

impl ReadPicoAdapter {
    #[must_use]
    pub fn info_request() -> HttpRequest {
        HttpRequest::new(HttpMethod::Get, "/info")
    }

    #[must_use]
    pub fn list_request(page: usize) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/books");
        request.query.push(("page".to_owned(), page.to_string()));
        request
    }

    #[must_use]
    pub fn upload_request(
        path: PathBuf,
        name: String,
        overwrite: bool,
        content_type: Option<String>,
    ) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Put, "/upload");
        request.query.push(("name".to_owned(), name));
        if overwrite {
            request.query.push(("overwrite".to_owned(), "1".to_owned()));
        }
        request.body = HttpBody::RawFile { path, content_type };
        request
    }

    #[must_use]
    pub fn delete_request(name: &str) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Delete, "/books");
        request.query.push(("name".to_owned(), name.to_owned()));
        request
    }

    /// Builds a request to save the Read Pico Wi-Fi credential.
    ///
    /// # Errors
    ///
    /// Returns an error if the credential cannot be serialized.
    pub fn wifi_save_request(credential: &WifiCredential) -> Result<HttpRequest, SdkError> {
        let body = serde_json::to_vec(&ReadPicoWifiCredential::from(credential))
            .map_err(|error| SdkError::InvalidArgument(format!("invalid Wi-Fi data: {error}")))?;
        let mut request = HttpRequest::new(HttpMethod::Post, "/wifi");
        request.body = HttpBody::Json(body);
        Ok(request)
    }

    #[must_use]
    pub fn wifi_delete_request() -> HttpRequest {
        HttpRequest::new(HttpMethod::Delete, "/wifi")
    }

    /// Parses one page from the Read Pico `/books` response.
    ///
    /// # Errors
    ///
    /// Returns an error when the location is not the active root or the response is invalid.
    pub fn parse_file_page(
        location: &FileLocation,
        body: &str,
    ) -> Result<AdapterFilePage, SdkError> {
        if !matches!(location, FileLocation::Root) {
            return Err(SdkError::Unsupported(
                "Read Pico does not support directory selection".to_owned(),
            ));
        }

        let response: ReadPicoBookPage = serde_json::from_str(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid Read Pico file list: {error}"))
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

        Ok(AdapterFilePage {
            entries,
            page: response.page,
            total: response.total,
            pages: response.pages,
        })
    }
}

#[derive(Serialize)]
struct ReadPicoWifiCredential<'a> {
    ssid: &'a str,
    password: &'a str,
}

impl<'a> From<&'a WifiCredential> for ReadPicoWifiCredential<'a> {
    fn from(credential: &'a WifiCredential) -> Self {
        Self {
            ssid: &credential.ssid,
            password: credential.password.as_deref().unwrap_or_default(),
        }
    }
}

impl ReadPicoAdapter {
    /// Parses saved Wi-Fi information from `/info`.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is not valid Read Pico device information.
    pub fn parse_wifi_info(body: &str) -> Result<Vec<WifiNetwork>, SdkError> {
        let info: ReadPicoInfo = serde_json::from_str(body)
            .map_err(|error| SdkError::RemoteFailure(format!("invalid Read Pico info: {error}")))?;
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
}

#[derive(Deserialize)]
struct ReadPicoInfo {
    wifi_configured: bool,
    wifi_ssid: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_book_page_into_common_entries() {
        let body = r#"{
            "page": 0,
            "total": 2,
            "pages": 1,
            "items": [
                {"name": "alpha.epub", "size": 1024},
                {"name": "notes.txt", "size": 20}
            ]
        }"#;

        let page = ReadPicoAdapter::parse_file_page(&FileLocation::Root, body).unwrap();

        assert_eq!(page.total, 2);
        assert_eq!(page.entries[0].path, "/alpha.epub");
        assert_eq!(page.entries[1].kind, FileKind::Book);
    }

    #[test]
    fn rejects_directory_location() {
        let result = ReadPicoAdapter::parse_file_page(
            &FileLocation::Directory("/Books".to_owned()),
            r#"{"page":0,"total":0,"pages":0,"items":[]}"#,
        );

        assert!(matches!(result, Err(SdkError::Unsupported(_))));
    }
}
