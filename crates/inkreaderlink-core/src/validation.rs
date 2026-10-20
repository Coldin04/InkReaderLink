//! Protocol-independent input validation and result helpers.
use crate::{FileKind, FileLocation, SdkError};
use std::{collections::HashSet, path::Path};
pub(crate) fn validate_file_name(name: &str) -> Result<(), SdkError> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(SdkError::InvalidArgument(
            "file name must be a single non-empty path component".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_remote_path(path: &str) -> Result<(), SdkError> {
    if !path.starts_with('/') || path.contains("/../") || path.ends_with("/..") {
        return Err(SdkError::InvalidArgument(
            "remote path must be absolute and must not traverse parents".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_batch_paths(paths: &[String]) -> Result<(), SdkError> {
    if paths.is_empty() {
        return Err(SdkError::InvalidArgument(
            "at least one file is required".to_owned(),
        ));
    }
    let mut unique = HashSet::with_capacity(paths.len());
    for path in paths {
        validate_remote_path(path)?;
        if !unique.insert(path.as_str()) {
            return Err(SdkError::InvalidArgument(format!(
                "duplicate remote path: {path}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn batch_operation_error(
    operation: &str,
    completed: usize,
    total: usize,
    path: &str,
    error: &SdkError,
) -> SdkError {
    if matches!(
        error,
        SdkError::CredentialRequired { .. } | SdkError::CredentialRejected { .. }
    ) {
        return error.clone();
    }
    SdkError::RemoteFailure(format!(
        "batch {operation} stopped after {completed}/{total} files; failed at {path}: {error}. Earlier completed files were not rolled back."
    ))
}

pub(crate) fn location_string(location: &FileLocation) -> String {
    match location {
        FileLocation::Root => "/".to_owned(),
        FileLocation::Directory(path) => path.clone(),
    }
}

pub(crate) fn join_location(location: &FileLocation, name: &str) -> String {
    let base = location_string(location);
    if base == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", base.trim_end_matches('/'))
    }
}

pub(crate) fn infer_file_kind(name: &str) -> FileKind {
    match Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("epub" | "txt" | "md" | "xtc") => FileKind::Book,
        _ => FileKind::Other,
    }
}
