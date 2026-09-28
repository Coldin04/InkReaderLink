#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Capability(String);

impl Capability {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.0
    }
}

pub mod ids {
    pub const FILE_LIST: &str = "files.list";
    pub const FILE_UPLOAD: &str = "files.upload";
    pub const FILE_DELETE: &str = "files.delete";
    pub const FILE_DOWNLOAD: &str = "files.download";
    pub const FILE_RENAME: &str = "files.rename";
    pub const FILE_MOVE: &str = "files.move";
    pub const DIRECTORY_CREATE: &str = "directories.create";
    pub const UPLOAD_EXPLICIT_OVERWRITE: &str = "upload.explicit-overwrite";
    pub const UPLOAD_WEBSOCKET: &str = "upload.websocket";
    pub const WIFI_MANAGE: &str = "wifi.manage";
}
