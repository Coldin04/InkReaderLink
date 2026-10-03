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
    pub const DEVICE_INFO: &str = "device.info";
    pub const FILE_LIST: &str = "files.list";
    pub const FILE_DIRECTORY_LIST: &str = "files.list.directories";
    pub const FILE_UPLOAD: &str = "files.upload";
    pub const UPLOAD_DIRECTORY_TARGET: &str = "upload.target-directory";
    pub const FILE_DELETE: &str = "files.delete";
    pub const FILE_DOWNLOAD: &str = "files.download";
    pub const FILE_RENAME: &str = "files.rename";
    pub const FILE_MOVE: &str = "files.move";
    pub const DIRECTORY_CREATE: &str = "directories.create";
    pub const UPLOAD_EXPLICIT_OVERWRITE: &str = "upload.explicit-overwrite";
    pub const UPLOAD_BACKUP_REPLACE: &str = "upload.backup-replace";
    pub const UPLOAD_WEBSOCKET: &str = "upload.websocket";
    pub const FONTS_UPLOAD_PROGRESS: &str = "fonts.upload.progress";
    pub const WIFI_LIST: &str = "wifi.list";
    pub const WIFI_SAVE: &str = "wifi.save";
    pub const WIFI_DELETE: &str = "wifi.delete";
    pub const FONTS_LIST: &str = "fonts.list";
    pub const FONTS_UPLOAD: &str = "fonts.upload";
    pub const FONTS_UPLOAD_FAMILY: &str = "fonts.upload.family";
    pub const FONTS_DELETE: &str = "fonts.delete";
    pub const OPDS_LIST: &str = "opds.list";
    pub const OPDS_SAVE: &str = "opds.save";
    pub const OPDS_DELETE: &str = "opds.delete";
    pub const SETTINGS_LIST: &str = "settings.list";
    pub const SETTINGS_UPDATE: &str = "settings.update";
    pub const WALLPAPERS_UPLOAD: &str = "wallpapers.upload";
    pub const WALLPAPERS_MANAGE: &str = "wallpapers.manage";
    pub const WALLPAPERS_DELETE: &str = "wallpapers.delete";
}
