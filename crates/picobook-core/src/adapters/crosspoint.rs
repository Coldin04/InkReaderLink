//! `CrossPoint` protocol adapter.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    FileEntry, FileKind, FileLocation, FontCatalog, OpdsCredential, OpdsServer, SdkError,
    WifiCredential, WifiNetwork,
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
    pub fn status_request() -> HttpRequest {
        HttpRequest::new(HttpMethod::Get, "/api/status")
    }

    /// Checks that a response has CrossPoint's status identity fields.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is not a CrossPoint status document.
    pub fn validate_status(body: &[u8]) -> Result<(), SdkError> {
        let status: serde_json::Value = serde_json::from_slice(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid CrossPoint device status: {error}"))
        })?;
        let valid = status.is_object()
            && status
                .get("version")
                .and_then(serde_json::Value::as_str)
                .is_some()
            && status
                .get("device")
                .and_then(serde_json::Value::as_str)
                .is_some();
        if valid {
            Ok(())
        } else {
            Err(SdkError::RemoteFailure(
                "invalid CrossPoint device status: required fields are missing".to_owned(),
            ))
        }
    }

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
            fields: Vec::new(),
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

    #[must_use]
    pub fn fonts_list_request() -> HttpRequest {
        HttpRequest::new(HttpMethod::Get, "/api/fonts")
    }

    #[must_use]
    pub fn font_upload_request(family: &str, path: PathBuf, file_name: String) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Post, "/api/fonts/upload");
        request.body = HttpBody::MultipartFile {
            path,
            file_name,
            field_name: "file".to_owned(),
            content_type: None,
            fields: vec![("family".to_owned(), family.to_owned())],
        };
        request
    }

    /// # Errors
    /// Returns an error if the family cannot be serialized.
    pub fn font_delete_request(family: &str) -> Result<HttpRequest, SdkError> {
        let body = serde_json::to_vec(&serde_json::json!({ "family": family }))
            .map_err(|error| SdkError::InvalidArgument(format!("invalid font family: {error}")))?;
        Ok(json_request(HttpMethod::Post, "/api/fonts/delete", body))
    }

    /// # Errors
    /// Returns an error for a malformed font catalog.
    pub fn parse_font_catalog(body: &str) -> Result<FontCatalog, SdkError> {
        serde_json::from_str(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid CrossPoint font catalog: {error}"))
        })
    }

    #[must_use]
    pub fn opds_list_request() -> HttpRequest {
        HttpRequest::new(HttpMethod::Get, "/api/opds")
    }

    /// # Errors
    /// Returns an error if the credential cannot be serialized.
    pub fn opds_save_request(credential: &OpdsCredential) -> Result<HttpRequest, SdkError> {
        let body = serde_json::to_vec(credential)
            .map_err(|error| SdkError::InvalidArgument(format!("invalid OPDS data: {error}")))?;
        Ok(json_request(HttpMethod::Post, "/api/opds", body))
    }

    /// # Errors
    /// Returns an error if the index cannot be serialized.
    pub fn opds_delete_request(index: u32) -> Result<HttpRequest, SdkError> {
        let body = serde_json::to_vec(&CrossPointWifiDelete { index })
            .map_err(|error| SdkError::InvalidArgument(format!("invalid OPDS index: {error}")))?;
        Ok(json_request(HttpMethod::Post, "/api/opds/delete", body))
    }

    /// # Errors
    /// Returns an error for a malformed OPDS server list.
    pub fn parse_opds_list(body: &str) -> Result<Vec<OpdsServer>, SdkError> {
        serde_json::from_str(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid CrossPoint OPDS list: {error}"))
        })
    }

    #[must_use]
    pub fn settings_list_request() -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/api/settings");
        request.response_limit = Some(512 * 1024);
        request
    }

    #[must_use]
    pub fn settings_update_request(body: Vec<u8>) -> HttpRequest {
        json_request(HttpMethod::Post, "/api/settings", body)
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
    use crate::transport::HttpBody;

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

    #[test]
    fn font_endpoints_match_crossmux_contract() {
        let catalog = CrossPointAdapter::parse_font_catalog(
            r#"{"maxFamilies":128,"families":[{"name":"Literata","sizes":[12,14],"files":[{"name":"Literata_12.cpfont","size":123}]}]}"#,
        ).unwrap();
        assert_eq!(catalog.max_families, 128);
        assert_eq!(catalog.families[0].files[0].size, 123);

        let request = CrossPointAdapter::font_upload_request(
            "Literata",
            PathBuf::from("/tmp/font.cpfont"),
            "Literata_12.cpfont".to_owned(),
        );
        assert_eq!(request.path, "/api/fonts/upload");
        assert!(
            matches!(request.body, HttpBody::MultipartFile { fields, field_name, .. }
            if field_name == "file" && fields == [("family".to_owned(), "Literata".to_owned())])
        );

        let request = CrossPointAdapter::font_delete_request("Literata").unwrap();
        assert_eq!(request.path, "/api/fonts/delete");
        assert!(matches!(request.body, HttpBody::Json(body)
            if serde_json::from_slice::<serde_json::Value>(&body).unwrap() == serde_json::json!({"family":"Literata"})));
    }

    #[test]
    fn opds_update_omits_password_when_preserving_existing_secret() {
        let servers = CrossPointAdapter::parse_opds_list(
            r#"[{"index":0,"name":"Catalog","url":"http://host/opds","username":"reader","hasPassword":true}]"#,
        ).unwrap();
        assert_eq!(servers[0].index, 0);
        assert!(servers[0].has_password);

        let request = CrossPointAdapter::opds_save_request(&OpdsCredential {
            index: Some(0),
            name: "Catalog".to_owned(),
            url: "http://host/opds".to_owned(),
            username: "reader".to_owned(),
            password: None,
        })
        .unwrap();
        assert_eq!(request.path, "/api/opds");
        assert!(matches!(request.body, HttpBody::Json(body) if {
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            value["index"] == 0 && value.get("password").is_none()
        }));

        let request = CrossPointAdapter::opds_delete_request(0).unwrap();
        assert_eq!(request.path, "/api/opds/delete");
    }
}
