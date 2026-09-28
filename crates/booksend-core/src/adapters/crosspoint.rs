//! `CrossPoint` protocol adapter.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    FileEntry, FileKind, FileLocation, SdkError, WifiCredential, WifiNetwork,
    transport::{HttpBody, HttpMethod, HttpRequest},
};

use super::AdapterFilePage;

/// `CrossPoint` protocol implementation.
#[derive(Debug, Default)]
pub struct CrossPointAdapter;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrossPointFile {
    name: String,
    size: u64,
    is_directory: bool,
    is_epub: bool,
}

impl CrossPointAdapter {
    #[must_use]
    pub fn list_request(location: &FileLocation) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/api/files");
        request
            .query
            .push(("path".to_owned(), location_path(location)));
        request
    }

    #[must_use]
    pub fn upload_request(
        location: &FileLocation,
        path: PathBuf,
        file_name: String,
        content_type: Option<String>,
    ) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Post, "/upload");
        request
            .query
            .push(("path".to_owned(), location_path(location)));
        request.body = HttpBody::MultipartFile {
            path,
            file_name,
            field_name: "file".to_owned(),
            content_type,
        };
        request
    }

    #[must_use]
    pub fn download_request(path: &str) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/download");
        request.query.push(("path".to_owned(), path.to_owned()));
        request
    }

    #[must_use]
    pub fn delete_request(path: &str) -> HttpRequest {
        form_request("/delete", [("path", path)])
    }

    #[must_use]
    pub fn rename_request(path: &str, name: &str) -> HttpRequest {
        form_request("/rename", [("path", path), ("name", name)])
    }

    #[must_use]
    pub fn move_request(path: &str, destination: &str) -> HttpRequest {
        form_request("/move", [("path", path), ("dest", destination)])
    }

    #[must_use]
    pub fn mkdir_request(parent: &str, name: &str) -> HttpRequest {
        form_request("/mkdir", [("path", parent), ("name", name)])
    }

    #[must_use]
    pub fn wifi_list_request() -> HttpRequest {
        HttpRequest::new(HttpMethod::Get, "/api/wifi")
    }

    /// Builds a request to add or update a Wi-Fi credential.
    ///
    /// # Errors
    ///
    /// Returns an error if the credential cannot be serialized.
    pub fn wifi_save_request(credential: &WifiCredential) -> Result<HttpRequest, SdkError> {
        let body = serde_json::to_vec(&CrossPointWifiCredential::from(credential))
            .map_err(|error| SdkError::InvalidArgument(format!("invalid Wi-Fi data: {error}")))?;
        Ok(json_request(HttpMethod::Post, "/api/wifi", body))
    }

    /// Builds a request to delete a Wi-Fi credential.
    ///
    /// # Errors
    ///
    /// Returns an error if the index cannot be serialized.
    pub fn wifi_delete_request(index: u32) -> Result<HttpRequest, SdkError> {
        let body = serde_json::to_vec(&CrossPointWifiDelete { index })
            .map_err(|error| SdkError::InvalidArgument(format!("invalid Wi-Fi index: {error}")))?;
        Ok(json_request(HttpMethod::Post, "/api/wifi/delete", body))
    }

    /// Parses the `CrossPoint` Wi-Fi list response.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is not a valid Wi-Fi list.
    pub fn parse_wifi_list(body: &str) -> Result<Vec<WifiNetwork>, SdkError> {
        serde_json::from_str::<Vec<CrossPointWifiNetwork>>(body)
            .map(|items| items.into_iter().map(Into::into).collect())
            .map_err(|error| {
                SdkError::RemoteFailure(format!("invalid CrossPoint Wi-Fi list: {error}"))
            })
    }

    /// Parses a `CrossPoint` `/api/files` response for a directory location.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is not a valid JSON file list.
    pub fn parse_file_list(
        location: &FileLocation,
        body: &str,
    ) -> Result<AdapterFilePage, SdkError> {
        let files: Vec<CrossPointFile> = serde_json::from_str(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid CrossPoint file list: {error}"))
        })?;
        let base = match location {
            FileLocation::Root => "/",
            FileLocation::Directory(path) => path.as_str(),
        };
        let entries = files
            .into_iter()
            .map(|file| FileEntry {
                path: join_device_path(base, &file.name),
                name: file.name,
                size: file.size,
                kind: if file.is_directory {
                    FileKind::Directory
                } else if file.is_epub {
                    FileKind::Book
                } else {
                    FileKind::Other
                },
            })
            .collect::<Vec<_>>();
        let total = entries.len();

        Ok(AdapterFilePage {
            entries,
            page: 0,
            total,
            pages: usize::from(total > 0),
        })
    }
}

fn location_path(location: &FileLocation) -> String {
    match location {
        FileLocation::Root => "/".to_owned(),
        FileLocation::Directory(path) => path.clone(),
    }
}

fn form_request<const N: usize>(path: &str, fields: [(&str, &str); N]) -> HttpRequest {
    let mut request = HttpRequest::new(HttpMethod::Post, path);
    request.body = HttpBody::Form(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
    );
    request
}

fn json_request(method: HttpMethod, path: &str, body: Vec<u8>) -> HttpRequest {
    let mut request = HttpRequest::new(method, path);
    request.body = HttpBody::Json(body);
    request
}

#[derive(Deserialize)]
struct CrossPointWifiNetwork {
    index: u32,
    ssid: String,
    #[serde(rename = "hasPassword")]
    has_password: bool,
    #[serde(rename = "isLastConnected")]
    is_last_connected: bool,
}

impl From<CrossPointWifiNetwork> for WifiNetwork {
    fn from(network: CrossPointWifiNetwork) -> Self {
        Self {
            index: Some(network.index),
            ssid: network.ssid,
            has_password: network.has_password,
            is_last_connected: network.is_last_connected,
        }
    }
}

#[derive(Serialize)]
struct CrossPointWifiCredential<'a> {
    ssid: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    index: &'a Option<u32>,
}

impl<'a> From<&'a WifiCredential> for CrossPointWifiCredential<'a> {
    fn from(credential: &'a WifiCredential) -> Self {
        Self {
            ssid: &credential.ssid,
            password: &credential.password,
            index: &credential.index,
        }
    }
}

#[derive(Serialize)]
struct CrossPointWifiDelete {
    index: u32,
}

fn join_device_path(base: &str, name: &str) -> String {
    if base == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", base.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_directory_listing_into_common_entries() {
        let body = r#"[
            {"name":"novel.epub","size":2048,"isDirectory":false,"isEpub":true},
            {"name":"Archive","size":0,"isDirectory":true,"isEpub":false},
            {"name":"cover.jpg","size":42,"isDirectory":false,"isEpub":false}
        ]"#;

        let page =
            CrossPointAdapter::parse_file_list(&FileLocation::Directory("/Books".to_owned()), body)
                .unwrap();

        assert_eq!(page.total, 3);
        assert_eq!(page.entries[0].path, "/Books/novel.epub");
        assert_eq!(page.entries[0].kind, FileKind::Book);
        assert_eq!(page.entries[1].kind, FileKind::Directory);
        assert_eq!(page.entries[2].kind, FileKind::Other);
    }
}
