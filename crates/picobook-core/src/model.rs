#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileKind {
    Book,
    Directory,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub kind: FileKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileLocation {
    Root,
    Directory(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceConstraints {
    pub can_list_directories: bool,
    pub can_choose_upload_directory: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceFileFormats {
    pub accepts_any_upload_format: bool,
    pub upload_extensions: Vec<String>,
    pub readable_extensions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictPolicy {
    Fail,
    OverwriteWhenSupported,
    ReplaceWithBackup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadOptions {
    pub conflict_policy: ConflictPolicy,
    pub content_type: Option<String>,
    pub prefer_websocket: bool,
}

impl Default for UploadOptions {
    fn default() -> Self {
        Self {
            conflict_policy: ConflictPolicy::Fail,
            content_type: None,
            prefer_websocket: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WifiNetwork {
    pub index: Option<u32>,
    pub ssid: String,
    pub has_password: bool,
    pub is_last_connected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WifiCredential {
    pub index: Option<u32>,
    pub ssid: String,
    pub password: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadResult {
    pub entry: FileEntry,
    pub used_websocket: bool,
}

pub trait UploadProgressSink: Send + Sync {
    fn report(&self, sent_bytes: u64, total_bytes: u64);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SdkError {
    InvalidArgument(String),
    Unsupported(String),
    Unreachable,
    Timeout,
    Conflict(String),
    InsufficientStorage,
    RemoteFailure(String),
    CommittedWithWarning(String),
    CommittedButCleanupFailed(String),
    RecoveryFailed(String),
}

impl std::fmt::Display for SdkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(formatter, "invalid argument: {message}"),
            Self::Unsupported(message) => write!(formatter, "unsupported: {message}"),
            Self::Unreachable => formatter.write_str("device unreachable"),
            Self::Timeout => formatter.write_str("operation timed out"),
            Self::Conflict(message) => write!(formatter, "conflict: {message}"),
            Self::InsufficientStorage => formatter.write_str("insufficient storage"),
            Self::RemoteFailure(message) => write!(formatter, "remote failure: {message}"),
            Self::CommittedWithWarning(message) => {
                write!(formatter, "committed with warning: {message}")
            }
            Self::CommittedButCleanupFailed(message) => {
                write!(formatter, "committed but cleanup failed: {message}")
            }
            Self::RecoveryFailed(message) => write!(formatter, "recovery failed: {message}"),
        }
    }
}

impl std::error::Error for SdkError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_upload_does_not_overwrite() {
        assert_eq!(
            UploadOptions::default().conflict_policy,
            ConflictPolicy::Fail
        );
    }
}
