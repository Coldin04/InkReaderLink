//! if-ink OS protocol extensions shared with the `KiikoRead` adapter.

use std::path::PathBuf;

use serde::Serialize;

use crate::{
    SdkError,
    transport::{HttpBody, HttpMethod, HttpRequest, HttpResponse},
};

#[derive(Debug, Default)]
pub struct IfInkOsAdapter;

impl IfInkOsAdapter {
    /// # Errors
    /// Returns a timeout, conflict, or remote failure for an unsuccessful HTTP response.
    pub fn ensure_success(response: &HttpResponse, operation: &str) -> Result<(), SdkError> {
        match response.status {
            200..=299 => Ok(()),
            408 => Err(SdkError::Timeout),
            409 => Err(SdkError::Conflict(response.text_lossy())),
            status => Err(SdkError::RemoteFailure(format!(
                "{operation} failed with HTTP {status}: {}",
                response.text_lossy()
            ))),
        }
    }

    #[must_use]
    pub fn file_upload_request(path: PathBuf, target: String, overwrite: bool) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Put, "/file-upload");
        request.query.push(("path".to_owned(), target));
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
    pub fn document_upload_request(path: PathBuf, name: String, overwrite: bool) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Put, "/document-upload");
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

    /// # Errors
    /// Returns an argument error if the firmware mutation cannot be serialized.
    pub fn create_directory_request(parent: &str, name: &str) -> Result<HttpRequest, SdkError> {
        let parent = relative_path(parent);
        let target = join_relative(parent, name);
        mutation_request("mkdir", parent, Some(&target))
    }

    /// # Errors
    /// Returns an argument error if the firmware mutation cannot be serialized.
    pub fn rename_request(path: &str, new_name: &str) -> Result<HttpRequest, SdkError> {
        let source = relative_path(path);
        let parent = source.rsplit_once('/').map_or("", |(parent, _)| parent);
        let target = join_relative(parent, new_name);
        mutation_request("rename", source, Some(&target))
    }

    /// # Errors
    /// Returns an argument error if the firmware mutation cannot be serialized.
    pub fn move_request(path: &str, destination: &str) -> Result<HttpRequest, SdkError> {
        let source = relative_path(path);
        let name = source.rsplit('/').next().unwrap_or(source);
        let target = join_relative(relative_path(destination), name);
        mutation_request("rename", source, Some(&target))
    }

    #[must_use]
    pub fn wifi_delete_request() -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Delete, "/wifi");
        request
            .headers
            .push(("Content-Type".to_owned(), "application/json".to_owned()));
        request
    }
}

#[derive(Serialize)]
struct FileMutation<'a> {
    action: &'a str,
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<&'a str>,
}

fn mutation_request(
    action: &str,
    path: &str,
    target: Option<&str>,
) -> Result<HttpRequest, SdkError> {
    let body = serde_json::to_vec(&FileMutation {
        action,
        path,
        target,
    })
    .map_err(|error| SdkError::InvalidArgument(format!("invalid file operation: {error}")))?;
    let mut request = HttpRequest::new(HttpMethod::Post, "/files");
    request.body = HttpBody::Json(body);
    Ok(request)
}

fn relative_path(path: &str) -> &str {
    path.trim_matches('/')
}

fn join_relative(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

pub(crate) mod session;

pub(crate) mod definition;

#[cfg(test)]
mod session_tests;
