//! `JustRead`'s authenticated SD-card transfer protocol.

use std::path::PathBuf;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;

use crate::{
    FileEntry, FileKind, FileLocation, SdkError,
    transport::{HttpBody, HttpMethod, HttpRequest},
};

pub const ACCESS_CODE_KEY: &str = "access_code";
const TOKEN_HEADER: &str = "X-Transfer-Token";

#[derive(Debug, Default)]
pub struct JustReadAdapter;

#[derive(Deserialize)]
struct DirectoryListing {
    entries: Vec<DirectoryEntry>,
    truncated: bool,
}

#[derive(Deserialize)]
struct DirectoryEntry {
    name: String,
    dir: bool,
    size: u64,
}

#[derive(Deserialize)]
struct DownloadTicket {
    url: String,
}

impl JustReadAdapter {
    /// Validates the response used for a connection probe. A truncated root is still valid.
    ///
    /// # Errors
    ///
    /// Returns an error if the response is not a directory listing.
    pub fn validate_listing(body: &[u8]) -> Result<(), SdkError> {
        let _: DirectoryListing = serde_json::from_slice(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid JustRead directory listing: {error}"))
        })?;
        Ok(())
    }

    /// Root lists the SD-card root; a directory is an absolute SDK path.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths or access codes.
    pub fn list_request(
        location: &FileLocation,
        access_code: Option<&str>,
    ) -> Result<HttpRequest, SdkError> {
        let directory = location_path(location)?;
        let mut request = HttpRequest::new(HttpMethod::Get, "/api/list");
        request.headers.push((
            "X-Transfer-Directory".to_owned(),
            STANDARD.encode(directory),
        ));
        add_access_code(&mut request, access_code)?;
        request.response_limit = Some(512 * 1024);
        Ok(request)
    }

    /// Parses a complete directory response.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, truncated, or unsafe listings.
    pub fn parse_listing(location: &FileLocation, body: &[u8]) -> Result<Vec<FileEntry>, SdkError> {
        let listing: DirectoryListing = serde_json::from_slice(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid JustRead directory listing: {error}"))
        })?;
        if listing.truncated {
            return Err(SdkError::RemoteFailure(
                "JustRead directory exceeds its 256-entry listing limit".to_owned(),
            ));
        }
        if listing.entries.len() > 256 {
            return Err(SdkError::RemoteFailure(
                "invalid JustRead directory listing: too many entries".to_owned(),
            ));
        }
        let directory = location_path(location)?;
        listing
            .entries
            .into_iter()
            .map(|entry| {
                validate_path(&entry.name, false)?;
                if entry.name.contains('/') {
                    return Err(SdkError::RemoteFailure(
                        "invalid JustRead directory entry name".to_owned(),
                    ));
                }
                let path = if directory.is_empty() {
                    format!("/{}", entry.name)
                } else {
                    format!("/{directory}/{}", entry.name)
                };
                let kind = if entry.dir {
                    FileKind::Directory
                } else if is_book(&entry.name) {
                    FileKind::Book
                } else {
                    FileKind::Other
                };
                Ok(FileEntry {
                    name: entry.name,
                    path,
                    size: entry.size,
                    kind,
                })
            })
            .collect()
    }

    /// `Root` uploads books into `/books`, the directory scanned by `JustRead`'s shelf.
    ///
    /// # Errors
    ///
    /// Returns an error for a name or destination rejected by the firmware's path rules.
    pub fn upload_path(location: &FileLocation, file_name: &str) -> Result<String, SdkError> {
        validate_path(file_name, false)?;
        if file_name.contains('/') {
            return Err(SdkError::InvalidArgument(
                "upload file name must be a single path component".to_owned(),
            ));
        }
        let parent = match location {
            FileLocation::Root => "books".to_owned(),
            FileLocation::Directory(_) => location_path(location)?,
        };
        let path = if parent.is_empty() {
            file_name.to_owned()
        } else {
            format!("{parent}/{file_name}")
        };
        validate_path(&path, false)?;
        Ok(format!("/{path}"))
    }

    /// Builds a streaming upload request.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid destination or access code.
    pub fn upload_request(
        local_path: PathBuf,
        destination: &str,
        access_code: Option<&str>,
    ) -> Result<HttpRequest, SdkError> {
        let relative = remote_path(destination)?;
        let mut request = HttpRequest::new(HttpMethod::Post, "/api/upload");
        request
            .headers
            .push(("X-Transfer-Path".to_owned(), STANDARD.encode(relative)));
        request.body = HttpBody::RawFile {
            path: local_path,
            content_type: None,
        };
        add_access_code(&mut request, access_code)?;
        request.response_limit = Some(1024);
        Ok(request)
    }

    /// Requests a one-use download ticket for a file.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid path or access code.
    pub fn download_ticket_request(
        path: &str,
        access_code: Option<&str>,
    ) -> Result<HttpRequest, SdkError> {
        let relative = remote_path(path)?;
        let mut request = HttpRequest::new(HttpMethod::Post, "/api/download-ticket");
        request
            .headers
            .push(("X-Transfer-Path".to_owned(), STANDARD.encode(relative)));
        add_access_code(&mut request, access_code)?;
        request.response_limit = Some(1024);
        Ok(request)
    }

    /// Converts a ticket response into a request bound to the device origin.
    ///
    /// # Errors
    ///
    /// Returns an error if the ticket URL is malformed or points elsewhere.
    pub fn download_request(body: &[u8]) -> Result<HttpRequest, SdkError> {
        let ticket: DownloadTicket = serde_json::from_slice(body).map_err(|error| {
            SdkError::RemoteFailure(format!("invalid JustRead download ticket: {error}"))
        })?;
        let value = ticket
            .url
            .strip_prefix("/api/download-file?t=")
            .ok_or_else(|| SdkError::RemoteFailure("invalid JustRead download URL".to_owned()))?;
        if value.len() != 12 || !value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Err(SdkError::RemoteFailure(
                "invalid JustRead download ticket".to_owned(),
            ));
        }
        let mut request = HttpRequest::new(HttpMethod::Get, "/api/download-file");
        request.query.push(("t".to_owned(), value.to_owned()));
        request.sensitive = true;
        Ok(request)
    }
}

fn add_access_code(request: &mut HttpRequest, code: Option<&str>) -> Result<(), SdkError> {
    if let Some(code) = code {
        request.sensitive = true;
        if code.is_empty() || code.len() > 32 || !code.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(SdkError::InvalidArgument(
                "invalid JustRead access code".to_owned(),
            ));
        }
        request
            .headers
            .push((TOKEN_HEADER.to_owned(), code.to_owned()));
    }
    Ok(())
}

fn remote_path(path: &str) -> Result<&str, SdkError> {
    let relative = path.strip_prefix('/').unwrap_or(path);
    validate_path(relative, false)?;
    Ok(relative)
}

fn location_path(location: &FileLocation) -> Result<String, SdkError> {
    match location {
        FileLocation::Root => Ok(String::new()),
        FileLocation::Directory(path) => {
            let relative = path.strip_prefix('/').unwrap_or(path);
            validate_path(relative, true)?;
            Ok(relative.to_owned())
        }
    }
}

fn validate_path(path: &str, allow_empty: bool) -> Result<(), SdkError> {
    if path.is_empty() && allow_empty {
        return Ok(());
    }
    if path.is_empty() || path.len() > 512 || path.ends_with('/') {
        return Err(SdkError::InvalidArgument(
            "invalid JustRead path".to_owned(),
        ));
    }
    for part in path.split('/') {
        let lower = part.to_ascii_lowercase();
        if part.is_empty()
            || part.len() > 255
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || lower == ".justread"
            || lower.starts_with(".jr-transfer-part-")
            || part
                .chars()
                .any(|character| character.is_control() || "\\:*?\"<>|".contains(character))
        {
            return Err(SdkError::InvalidArgument(
                "invalid JustRead path".to_owned(),
            ));
        }
    }
    Ok(())
}

fn is_book(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        extension.eq_ignore_ascii_case("epub") || extension.eq_ignore_ascii_case("txt")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_directory_and_rejects_incomplete_or_unsafe_listings() {
        let entries = JustReadAdapter::parse_listing(
            &FileLocation::Directory("/books".to_owned()),
            r#"{"entries":[{"name":"书.epub","dir":false,"size":42}],"truncated":false}"#
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(entries[0].path, "/books/书.epub");
        assert_eq!(entries[0].kind, FileKind::Book);
        assert!(
            JustReadAdapter::parse_listing(
                &FileLocation::Root,
                br#"{"entries":[],"truncated":true}"#,
            )
            .is_err()
        );
        JustReadAdapter::validate_listing(br#"{"entries":[],"truncated":true}"#).unwrap();
        assert!(
            JustReadAdapter::parse_listing(
                &FileLocation::Root,
                br#"{"entries":[{"name":"../bad","dir":false,"size":0}],"truncated":false}"#,
            )
            .is_err()
        );
    }

    #[test]
    fn validates_download_ticket_as_local_one_use_endpoint() {
        let request =
            JustReadAdapter::download_request(br#"{"url":"/api/download-file?t=ABCD2345EFGH"}"#)
                .unwrap();
        assert_eq!(request.path, "/api/download-file");
        assert_eq!(request.query, [("t".to_owned(), "ABCD2345EFGH".to_owned())]);
        assert!(
            JustReadAdapter::download_request(
                br#"{"url":"https://example.com/file?t=ABCD2345EFGH"}"#,
            )
            .is_err()
        );
    }
}

pub(crate) mod session;

pub(crate) mod definition;

#[cfg(test)]
mod session_tests;
