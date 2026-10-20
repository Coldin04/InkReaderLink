//! `FolioBook` browser uploader: raw file POSTs with a query-string filename.

use std::path::PathBuf;

use crate::{
    SdkError,
    transport::{HttpBody, HttpMethod, HttpRequest, HttpResponse},
};

#[derive(Debug, Default)]
pub struct FolioBookAdapter;

impl FolioBookAdapter {
    #[must_use]
    pub fn page_request() -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Get, "/");
        request.response_limit = Some(2 * 1024 * 1024);
        request
    }

    /// Identifies the supplied `FolioBook` uploader without attempting a write.
    ///
    /// # Errors
    /// Returns an error for HTTP failures or a different device's page.
    pub fn validate_page(response: &HttpResponse) -> Result<(), SdkError> {
        Self::ensure_success(response, "verify FolioBook connection")?;
        let body = response.text_lossy();
        let lower = body.to_ascii_lowercase();
        if !lower.contains("<title>folio book</title>")
            || !body.contains("/up?name=")
            || !body.contains("encodeURIComponent")
        {
            return Err(SdkError::RemoteFailure(
                "address does not serve a FolioBook uploader".to_owned(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn upload_request(
        local_path: PathBuf,
        file_name: &str,
        content_type: Option<String>,
    ) -> HttpRequest {
        let mut request = HttpRequest::new(HttpMethod::Post, "/up");
        request
            .query
            .push(("name".to_owned(), file_name.to_owned()));
        request.body = HttpBody::RawFile {
            path: local_path,
            content_type,
        };
        request.response_limit = Some(64 * 1024);
        request
    }

    /// Mirrors the browser's exact HTTP 200 success rule, preserving device errors.
    ///
    /// # Errors
    /// Returns `RemoteFailure` for every other HTTP status.
    pub fn ensure_success(response: &HttpResponse, operation: &str) -> Result<(), SdkError> {
        if response.status == 200 {
            return Ok(());
        }
        Err(SdkError::RemoteFailure(format!(
            "{operation} failed with HTTP {}: {}",
            response.status,
            response.text_lossy().trim()
        )))
    }
}

pub(crate) mod definition;
pub(crate) mod session;

#[cfg(test)]
mod session_tests;
