//! Internal capability-oriented interface. Firmware sessions own protocol state and I/O sequences.
use crate::validation::{batch_operation_error, infer_file_kind};
use crate::{
    DeviceInfoField, FileEntry, FileLocation, FontCatalog, OpdsCredential, OpdsServer, SdkError,
    SettingChange, SettingsSnapshot, SettingsValidationPolicy, UploadOptions, UploadProgressSink,
    UploadResult, WallpaperUploadResult, WifiCredential, WifiNetwork,
};
use async_trait::async_trait;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub(crate) struct FileUpload {
    pub local_path: PathBuf,
    pub file_name: String,
    pub location: FileLocation,
    pub options: UploadOptions,
    pub size: u64,
    pub progress: Option<Arc<dyn UploadProgressSink>>,
}
impl FileUpload {
    pub fn result(&self, path: String, used_websocket: bool) -> UploadResult {
        UploadResult {
            entry: FileEntry {
                name: self.file_name.clone(),
                path,
                size: self.size,
                kind: infer_file_kind(&self.file_name),
            },
            used_websocket,
        }
    }
}
pub(crate) struct FontUpload {
    pub family: String,
    pub local_path: PathBuf,
    pub file_name: String,
    pub overwrite: bool,
    pub size: u64,
    pub progress: Option<Arc<dyn UploadProgressSink>>,
}
pub(crate) struct WallpaperUpload {
    pub local_path: PathBuf,
    pub file_name: String,
    pub overwrite: bool,
    pub apply_to_lock_screen: bool,
    pub size: u64,
    pub progress: Option<Arc<dyn UploadProgressSink>>,
}
#[async_trait]
pub(crate) trait DeviceAdapter: Send + Sync {
    fn configure_address(&mut self, _address: &str) -> Result<(), SdkError> {
        Ok(())
    }

    fn set_session_credentials(&mut self, _credentials: HashMap<String, String>) {}
    async fn verify_connection(&self) -> Result<(), SdkError>;
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError>;
    fn validate_delete(&self, _path: &str) -> Result<(), SdkError> {
        Ok(())
    }
    async fn delete_files(&self, paths: &[String]) -> Result<(), SdkError> {
        for path in paths {
            self.validate_delete(path)?;
        }
        for (index, path) in paths.iter().enumerate() {
            if let Err(error) = self.delete(path).await {
                return Err(batch_operation_error(
                    "delete",
                    index,
                    paths.len(),
                    path,
                    &error,
                ));
            }
        }
        Ok(())
    }
    async fn device_info(&self) -> Result<Vec<DeviceInfoField>, SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement device info".to_owned(),
        ))
    }
    async fn list_files(&self, _location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement list files".to_owned(),
        ))
    }
    async fn delete(&self, _path: &str) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement delete".to_owned(),
        ))
    }
    async fn download(&self, _path: &str, _destination: PathBuf) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement download".to_owned(),
        ))
    }
    async fn create_directory(&self, _parent: &str, _name: &str) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement create directory".to_owned(),
        ))
    }
    async fn rename(&self, _path: &str, _new_name: &str) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement rename".to_owned(),
        ))
    }
    async fn move_file(&self, _path: &str, _destination: &str) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement move file".to_owned(),
        ))
    }
    async fn list_wifi_networks(&self) -> Result<Vec<WifiNetwork>, SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement list wifi networks".to_owned(),
        ))
    }
    async fn save_wifi_network(&self, _credential: &WifiCredential) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement save wifi network".to_owned(),
        ))
    }
    async fn delete_wifi_network(&self, _index: Option<u32>) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement delete wifi network".to_owned(),
        ))
    }
    async fn list_wallpapers(&self) -> Result<Vec<FileEntry>, SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement list wallpapers".to_owned(),
        ))
    }
    async fn upload_wallpaper(
        &self,
        _upload: WallpaperUpload,
    ) -> Result<WallpaperUploadResult, SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement upload wallpaper".to_owned(),
        ))
    }
    async fn delete_wallpaper(&self, _file_name: &str) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement delete wallpaper".to_owned(),
        ))
    }
    async fn list_fonts(&self) -> Result<FontCatalog, SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement list fonts".to_owned(),
        ))
    }
    async fn upload_font(&self, _upload: FontUpload) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement upload font".to_owned(),
        ))
    }
    async fn delete_font_family(&self, _family: &str) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement delete font family".to_owned(),
        ))
    }
    async fn list_opds_servers(&self) -> Result<Vec<OpdsServer>, SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement list opds servers".to_owned(),
        ))
    }
    async fn save_opds_server(&self, _credential: &OpdsCredential) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement save opds server".to_owned(),
        ))
    }
    async fn delete_opds_server(&self, _index: u32) -> Result<(), SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement delete opds server".to_owned(),
        ))
    }
    async fn list_settings(
        &self,
        _policy: SettingsValidationPolicy,
    ) -> Result<SettingsSnapshot, SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement list settings".to_owned(),
        ))
    }
    async fn apply_settings(
        &self,
        _expected: SettingsSnapshot,
        _changes: Vec<SettingChange>,
        _policy: SettingsValidationPolicy,
    ) -> Result<SettingsSnapshot, SdkError> {
        Err(SdkError::Unsupported(
            "adapter does not implement apply settings".to_owned(),
        ))
    }
}
