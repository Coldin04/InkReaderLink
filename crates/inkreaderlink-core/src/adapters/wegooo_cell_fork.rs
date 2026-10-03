//! `wegooo-cell-fork` protocol adapter.

use std::{path::Path, path::PathBuf};

use serde::Deserialize;

use crate::{
    FileEntry, FileKind, FileLocation, SdkError, WifiCredential, WifiNetwork,
    transport::{HttpBody, HttpMethod, HttpRequest},
};

use super::AdapterFilePage;

const WALLPAPER_DIRECTORY: &str = "pictures";
const WALLPAPER_EXTENSIONS: [&str; 3] = ["jpg", "jpeg", "png"];

#[derive(Debug, Default)]
pub struct WegoCellForkAdapter;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileListResponse {
    page: usize,
    total: usize,
    pages: usize,
    items: Vec<FileListItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileListItem {
    name: String,
    size: u64,
    #[serde(rename = "directory", alias = "isDirectory")]
    is_directory: bool,
}

impl WegoCellForkAdapter {
    #[must_use]
    pub fn info_request() -> HttpRequest {
        HttpRequest::new(HttpMethod::Get, "/info")
    }

    pub fn validate_info(body: &[u8]) -> Result<(), SdkError> {
        let info: serde_json::Value = serde_json::from_slice(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid wegooo-cell-fork device info: {error}"))
        })?;
        let valid = info.is_object()
            && info
                .get("is_flash")
                .and_then(serde_json::Value::as_bool)
                .is_some()
            && info
                .get("free_bytes")
                .and_then(serde_json::Value::as_u64)
                .is_some()
            && info
                .get("file_limit")
                .and_then(serde_json::Value::as_u64)
                .is_some()
            && info
                .get("mode")
                .and_then(serde_json::Value::as_str)
                .is_some()
            && info
                .get("wifi_configured")
                .and_then(serde_json::Value::as_bool)
                .is_some()
            && info
                .get("root")
                .and_then(serde_json::Value::as_str)
                .is_some();
        if valid {
            Ok(())
        } else {
            Err(SdkError::RemoteFailure(
                "invalid wegooo-cell-fork device info: required fields are missing".to_owned(),
            ))
        }
    }

    pub fn parse_info(body: &[u8]) -> Result<Vec<crate::DeviceInfoField>, SdkError> {
        Self::validate_info(body)?;
        let info: serde_json::Value = serde_json::from_slice(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid wegooo-cell-fork device info: {error}"))
        })?;
        Ok([
            ("is_flash", "storage_is_flash"),
            ("free_bytes", "storage_free_bytes"),
            ("file_limit", "storage_file_limit"),
            ("mode", "network_mode"),
            ("wifi_configured", "wifi_configured"),
            ("wifi_ssid", "wifi_ssid"),
            ("root", "storage_root"),
        ]
        .into_iter()
        .filter_map(|(wire_key, key)| {
            info.get(wire_key)
                .filter(|value| !value.is_null())
                .map(|value| crate::DeviceInfoField {
                    key: key.to_owned(),
                    value: value
                        .as_str()
                        .map_or_else(|| value.to_string(), str::to_owned),
                })
        })
        .collect())
    }

    pub fn parse_wifi_info(body: &str) -> Result<Vec<WifiNetwork>, SdkError> {
        let info: ForkWifiInfo = serde_json::from_str(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid wegooo-cell-fork Wi-Fi info: {error}"))
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

    pub fn wifi_save_request(credential: &WifiCredential) -> Result<HttpRequest, SdkError> {
        let body = serde_json::to_vec(&ForkWifiCredential {
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

    #[must_use]
    pub fn file_list_request(location: &FileLocation, page: usize) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/files");
        let path = location_path(location);
        if !path.is_empty() {
            request.query.push(("path".to_owned(), path));
        }
        request.query.push(("page".to_owned(), page.to_string()));
        request
    }

    pub fn parse_file_page(
        location: &FileLocation,
        body: &str,
    ) -> Result<AdapterFilePage, SdkError> {
        let response: FileListResponse = serde_json::from_str(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid wegooo-cell-fork file list: {error}"))
        })?;
        let base = location_path(location);
        let entries = response
            .items
            .into_iter()
            .map(|item| {
                let path = if base.is_empty() {
                    format!("/{}", item.name)
                } else {
                    format!("/{base}/{}", item.name)
                };
                let kind = if item.is_directory {
                    FileKind::Directory
                } else {
                    infer_file_kind(&item.name)
                };
                FileEntry {
                    name: item.name,
                    path,
                    size: item.size,
                    kind,
                }
            })
            .collect();
        Ok(AdapterFilePage {
            entries,
            page: response.page,
            total: response.total,
            pages: response.pages,
        })
    }

    #[must_use]
    pub fn book_upload_request(path: PathBuf, name: String, overwrite: bool) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Put, "/upload");
        request.query.push(("name".to_owned(), name));
        if overwrite {
            request.query.push(("overwrite".to_owned(), "1".to_owned()));
        }
        request.body = HttpBody::RawFile {
            path,
            content_type: None,
        };
        request
    }

    #[must_use]
    pub fn font_upload_request(path: PathBuf, name: String, overwrite: bool) -> HttpRequest {
        let content_type = match Path::new(&name)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("otf") => "font/otf",
            _ => "font/ttf",
        };
        let mut request = HttpRequest::new(HttpMethod::Put, "/font-upload");
        request.query.push(("name".to_owned(), name));
        if overwrite {
            request.query.push(("overwrite".to_owned(), "1".to_owned()));
        }
        request.body = HttpBody::RawFile {
            path,
            content_type: Some(content_type.to_owned()),
        };
        request
    }

    #[must_use]
    pub fn wallpaper_upload_request(
        path: PathBuf,
        name: String,
        overwrite: bool,
        apply_to_lock_screen: bool,
    ) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Put, "/image-upload");
        request.query.push(("name".to_owned(), name));
        if overwrite {
            request.query.push(("overwrite".to_owned(), "1".to_owned()));
        }
        if apply_to_lock_screen {
            request.query.push(("wallpaper".to_owned(), "1".to_owned()));
        }
        request.body = HttpBody::RawFile {
            path,
            content_type: None,
        };
        request
    }

    #[must_use]
    pub fn delete_path_request(path: &str) -> Result<HttpRequest, SdkError> {
        let body = serde_json::to_vec(&FileMutation {
            action: "delete",
            path,
            target: None,
        })
        .map_err(|error| SdkError::InvalidArgument(format!("invalid file path: {error}")))?;
        let mut request = HttpRequest::new(HttpMethod::Post, "/files");
        request.body = HttpBody::Json(body);
        Ok(request)
    }

    #[must_use]
    pub fn delete_wallpaper_request(name: &str) -> Result<HttpRequest, SdkError> {
        Self::delete_path_request(&format!("{WALLPAPER_DIRECTORY}/{name}"))
    }

    #[must_use]
    pub fn wallpapers_list_request(page: usize) -> HttpRequest {
        Self::file_list_request(
            &FileLocation::Directory(WALLPAPER_DIRECTORY.to_owned()),
            page,
        )
    }

    pub fn parse_wallpaper_page(body: &str) -> Result<AdapterFilePage, SdkError> {
        let mut page = Self::parse_file_page(
            &FileLocation::Directory(WALLPAPER_DIRECTORY.to_owned()),
            body,
        )?;
        page.entries
            .retain(|entry| entry.kind != FileKind::Directory && is_wallpaper_name(&entry.name));
        Ok(page)
    }
}

#[derive(serde::Serialize)]
struct FileMutation<'a> {
    action: &'a str,
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<&'a str>,
}

#[derive(Deserialize)]
struct ForkWifiInfo {
    wifi_configured: bool,
    wifi_ssid: Option<String>,
}

#[derive(serde::Serialize)]
struct ForkWifiCredential<'a> {
    ssid: &'a str,
    password: &'a str,
}

fn location_path(location: &FileLocation) -> String {
    match location {
        FileLocation::Root => String::new(),
        FileLocation::Directory(path) => path.trim_matches('/').to_owned(),
    }
}

fn infer_file_kind(name: &str) -> FileKind {
    match Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("epub" | "txt") => FileKind::Book,
        _ => FileKind::Other,
    }
}

fn is_wallpaper_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            WALLPAPER_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_list_accepts_firmware_directory_field() {
        let body = r#"{"path":"","total":2,"page":0,"pages":1,"items":[{"name":"books","directory":true,"size":0},{"name":"cover.jpg","directory":false,"size":237355}]}"#;
        let page = WegoCellForkAdapter::parse_file_page(&FileLocation::Root, body).unwrap();

        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].kind, FileKind::Directory);
        assert_eq!(page.entries[0].path, "/books");
        assert_eq!(page.entries[1].kind, FileKind::Other);
        assert_eq!(page.entries[1].path, "/cover.jpg");
    }

    #[test]
    fn wallpaper_list_accepts_firmware_directory_field() {
        let body = r#"{"path":"pictures","total":1,"page":0,"pages":1,"items":[{"name":"442260.jpg","directory":false,"size":237355}]}"#;
        let page = WegoCellForkAdapter::parse_wallpaper_page(body).unwrap();

        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].name, "442260.jpg");
        assert_eq!(page.entries[0].path, "/pictures/442260.jpg");
        assert_eq!(page.entries[0].size, 237355);
        assert_eq!(page.entries[0].kind, FileKind::Other);
    }
}
