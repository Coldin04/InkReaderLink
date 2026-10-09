//! WhiteOS HTTP file-management API adapter.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{
    FileEntry, FileKind, FileLocation,
    transport::{HttpBody, HttpMethod, HttpRequest},
};

use super::AdapterFilePage;

const REQUEST_HEADER: (&str, &str) = ("X-Pico-Request", "1");

#[derive(Debug, Default)]
pub struct WhiteOsAdapter;

#[derive(Deserialize)]
struct FileListResponse {
    path: String,
    entries: Vec<FileListEntry>,
}

#[derive(Deserialize)]
struct FileListEntry {
    name: String,
    dir: bool,
    size: u64,
}

impl WhiteOsAdapter {
    #[must_use]
    pub fn list_request(location: &FileLocation) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/api/list");
        request
            .query
            .push(("path".to_owned(), location_path(location)));
        request
    }

    /// Validates a root listing as the connection handshake.
    ///
    /// # Errors
    ///
    /// Returns an error if the response is not a WhiteOS file-list document.
    pub fn validate_file_list(body: &[u8]) -> Result<(), crate::SdkError> {
        let body = std::str::from_utf8(body).map_err(|error| {
            crate::SdkError::RemoteFailure(format!("invalid WhiteOS file list: {error}"))
        })?;
        Self::parse_file_list(body).map(|_| ())
    }

    /// Converts a WhiteOS listing into the common file-page model.
    ///
    /// WhiteOS returns the complete directory in one response, without page
    /// metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is invalid or contains an unsafe path.
    pub fn parse_file_list(body: &str) -> Result<AdapterFilePage, crate::SdkError> {
        let response: FileListResponse = serde_json::from_str(body).map_err(|error| {
            crate::SdkError::RemoteFailure(format!("invalid WhiteOS file list: {error}"))
        })?;
        validate_device_path(&response.path)?;

        let entries = response
            .entries
            .into_iter()
            .map(|entry| {
                if entry.name.is_empty()
                    || entry.name == "."
                    || entry.name == ".."
                    || entry.name.contains(['/', '\\'])
                {
                    return Err(crate::SdkError::RemoteFailure(
                        "invalid WhiteOS file list: entry name is not a path component".to_owned(),
                    ));
                }

                let path = if response.path == "/" {
                    format!("/{}", entry.name)
                } else {
                    format!("{}/{}", response.path.trim_end_matches('/'), entry.name)
                };
                let kind = if entry.dir {
                    FileKind::Directory
                } else {
                    infer_file_kind(&entry.name)
                };
                Ok(FileEntry {
                    name: entry.name,
                    path,
                    size: entry.size,
                    kind,
                })
            })
            .collect::<Result<Vec<_>, crate::SdkError>>()?;
        let total = entries.len();

        Ok(AdapterFilePage {
            entries,
            page: 0,
            total,
            pages: usize::from(total > 0),
        })
    }

    #[must_use]
    pub fn upload_request(
        local_path: PathBuf,
        destination: &str,
        content_type: Option<String>,
    ) -> HttpRequest {
        let mut request = mutating_request(HttpMethod::Put, "/api/upload");
        request
            .query
            .push(("path".to_owned(), destination.to_owned()));
        request.body = HttpBody::RawFile {
            path: local_path,
            content_type,
        };
        request
    }

    #[must_use]
    pub fn download_request(path: &str) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/api/download");
        request.query.push(("path".to_owned(), path.to_owned()));
        request
    }

    #[must_use]
    pub fn delete_request(path: &str) -> HttpRequest {
        let mut request = mutating_request(HttpMethod::Post, "/api/delete");
        request.query.push(("path".to_owned(), path.to_owned()));
        request
    }

    #[must_use]
    pub fn rename_request(from: &str, to: &str) -> HttpRequest {
        let mut request = mutating_request(HttpMethod::Post, "/api/rename");
        request.query.push(("from".to_owned(), from.to_owned()));
        request.query.push(("to".to_owned(), to.to_owned()));
        request
    }

    #[must_use]
    pub fn create_directory_request(path: &str) -> HttpRequest {
        let mut request = mutating_request(HttpMethod::Post, "/api/mkdir");
        request.query.push(("path".to_owned(), path.to_owned()));
        request
    }
}

fn mutating_request(method: HttpMethod, endpoint: &str) -> HttpRequest {
    let mut request = HttpRequest::new(method, endpoint);
    request
        .headers
        .push((REQUEST_HEADER.0.to_owned(), REQUEST_HEADER.1.to_owned()));
    request
}

fn location_path(location: &FileLocation) -> String {
    match location {
        FileLocation::Root => "/".to_owned(),
        FileLocation::Directory(path) if path.starts_with('/') => path.clone(),
        FileLocation::Directory(path) => format!("/{path}"),
    }
}

fn validate_device_path(path: &str) -> Result<(), crate::SdkError> {
    if !path.starts_with('/') || path.split('/').any(|segment| segment == "..") {
        return Err(crate::SdkError::RemoteFailure(
            "invalid WhiteOS file list: directory path must be absolute".to_owned(),
        ));
    }
    Ok(())
}

fn infer_file_kind(name: &str) -> FileKind {
    match Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("epub" | "txt" | "md" | "xtc") => FileKind::Book,
        _ => FileKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_directory_listing() {
        let body = r#"{
            "path": "/Books",
            "entries": [
                {"name": "小说.epub", "dir": false, "size": 1024},
                {"name": "archive", "dir": true, "size": 0}
            ]
        }"#;

        let page = WhiteOsAdapter::parse_file_list(body).unwrap();

        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].path, "/Books/小说.epub");
        assert_eq!(page.entries[0].kind, FileKind::Book);
        assert_eq!(page.entries[1].path, "/Books/archive");
        assert_eq!(page.entries[1].kind, FileKind::Directory);
        assert_eq!(page.total, 2);
        assert_eq!(page.pages, 1);
    }

    #[test]
    fn rejects_unsafe_listing_paths_and_names() {
        let traversal = r#"{"path":"/Books/../","entries":[]}"#;
        let invalid_name =
            r#"{"path":"/Books","entries":[{"name":"..\\secret","dir":false,"size":1}]}"#;

        assert!(WhiteOsAdapter::parse_file_list(traversal).is_err());
        assert!(WhiteOsAdapter::parse_file_list(invalid_name).is_err());
    }

    #[test]
    fn mutating_requests_include_the_required_header() {
        let requests = [
            WhiteOsAdapter::upload_request(
                PathBuf::from("/tmp/book.epub"),
                "/Books/book.epub",
                None,
            ),
            WhiteOsAdapter::delete_request("/Books/book.epub"),
            WhiteOsAdapter::rename_request("/Books/old.epub", "/Books/new.epub"),
            WhiteOsAdapter::create_directory_request("/Books/new"),
        ];

        for request in requests {
            assert!(
                request
                    .headers
                    .iter()
                    .any(|(name, value)| name == REQUEST_HEADER.0 && value == REQUEST_HEADER.1)
            );
        }
    }
}
