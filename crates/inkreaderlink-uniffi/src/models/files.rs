use inkreaderlink_core::{
    ConflictPolicy, FileDownload, FileEntry, FileKind, FileLocation, UploadOptions, UploadResult,
    WallpaperUploadResult,
};
use std::path::PathBuf;

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkConflictPolicy {
    Fail,
    OverwriteWhenSupported,
    ReplaceWithBackup,
}

impl From<SdkConflictPolicy> for ConflictPolicy {
    fn from(policy: SdkConflictPolicy) -> Self {
        match policy {
            SdkConflictPolicy::Fail => Self::Fail,
            SdkConflictPolicy::OverwriteWhenSupported => Self::OverwriteWhenSupported,
            SdkConflictPolicy::ReplaceWithBackup => Self::ReplaceWithBackup,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkUploadOptions {
    pub conflict_policy: SdkConflictPolicy,
    pub content_type: Option<String>,
    pub prefer_websocket: bool,
}

impl From<SdkUploadOptions> for UploadOptions {
    fn from(options: SdkUploadOptions) -> Self {
        Self {
            conflict_policy: options.conflict_policy.into(),
            content_type: options.content_type,
            prefer_websocket: options.prefer_websocket,
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkFileKind {
    Book,
    Directory,
    Other,
}

impl From<FileKind> for SdkFileKind {
    fn from(kind: FileKind) -> Self {
        match kind {
            FileKind::Book => Self::Book,
            FileKind::Directory => Self::Directory,
            FileKind::Other => Self::Other,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkFileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub kind: SdkFileKind,
}

impl From<FileEntry> for SdkFileEntry {
    fn from(entry: FileEntry) -> Self {
        Self {
            name: entry.name,
            path: entry.path,
            size: entry.size,
            kind: entry.kind.into(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkFileDownload {
    pub path: String,
    pub destination: String,
}

impl From<SdkFileDownload> for FileDownload {
    fn from(file: SdkFileDownload) -> Self {
        Self {
            path: file.path,
            destination: PathBuf::from(file.destination),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkUploadResult {
    pub entry: SdkFileEntry,
    pub used_websocket: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkWallpaperUploadResult {
    pub entry: SdkFileEntry,
    pub applied_to_lock_screen: bool,
}

impl From<WallpaperUploadResult> for SdkWallpaperUploadResult {
    fn from(result: WallpaperUploadResult) -> Self {
        Self {
            entry: result.entry.into(),
            applied_to_lock_screen: result.applied_to_lock_screen,
        }
    }
}

impl From<UploadResult> for SdkUploadResult {
    fn from(result: UploadResult) -> Self {
        Self {
            entry: result.entry.into(),
            used_websocket: result.used_websocket,
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkFileLocation {
    Root,
    Directory { path: String },
}

impl From<SdkFileLocation> for FileLocation {
    fn from(location: SdkFileLocation) -> Self {
        match location {
            SdkFileLocation::Root => Self::Root,
            SdkFileLocation::Directory { path } => Self::Directory(path),
        }
    }
}
