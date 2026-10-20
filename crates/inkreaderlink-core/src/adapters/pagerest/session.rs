//! Stateful `PageRestAdapter` protocol operations.
use crate::adapters::{DeviceAdapter, FileUpload, FontUpload, WallpaperUpload};
use crate::model::ConflictPolicy;
use crate::transport::{HttpRequest, SharedTransport};
use crate::validation::{join_location, validate_file_name};
use crate::{
    DeviceInfoField, FileEntry, FileKind, FileLocation, FontCatalog, FontFamily, FontFile,
    SdkError, UploadOptions, UploadResult, WallpaperUploadResult, WifiCredential, WifiNetwork,
};
use async_trait::async_trait;

use super::PageRestAdapter;
use super::PageRestUploadKind;
use crate::adapters::read_pico::session::read_pico_name;
pub(crate) struct Session {
    transport: SharedTransport,
}
impl Session {
    pub fn new(transport: SharedTransport) -> Self {
        Self { transport }
    }
}
impl Session {
    async fn list_pagerest(&self, location: FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        if !matches!(location, FileLocation::Root) {
            return Err(SdkError::Unsupported(
                "PageRest does not support directory selection".to_owned(),
            ));
        }
        let mut page_index = 0;
        let mut entries = Vec::new();
        loop {
            let response = self
                .transport
                .execute(PageRestAdapter::book_list_request(page_index))
                .await?;
            PageRestAdapter::ensure_success(&response, "list files")?;
            let page = PageRestAdapter::parse_book_page(&location, &response.text_lossy())?;
            entries.extend(page.entries);
            page_index += 1;
            if page_index >= page.pages {
                return Ok(entries);
            }
        }
    }
    async fn ensure_pagerest_ap_mode(
        &self,
    ) -> Result<crate::adapters::pagerest::PageRestInfo, SdkError> {
        let response = self
            .transport
            .execute(PageRestAdapter::info_request())
            .await?;
        PageRestAdapter::ensure_success(&response, "check PageRest Wi-Fi mode")?;
        let info = PageRestAdapter::parse_info(&response.body)?;
        if info.mode != "ap" {
            return Err(SdkError::Unsupported(
                "PageRest Wi-Fi settings can only be changed in device hotspot mode".to_owned(),
            ));
        }
        Ok(info)
    }
    async fn upload_pagerest(
        &self,
        upload: FileUpload,
        kind: PageRestUploadKind,
    ) -> Result<(), SdkError> {
        let FileUpload {
            local_path,
            file_name,
            location,
            options,
            size,
            progress,
        } = upload;
        let file_name = file_name.as_str();
        if kind == PageRestUploadKind::Book && !matches!(location, FileLocation::Root) {
            return Err(SdkError::Unsupported(
                "PageRest does not support directory selection".to_owned(),
            ));
        }

        let info_response = self
            .transport
            .execute(PageRestAdapter::info_request())
            .await?;
        PageRestAdapter::ensure_success(&info_response, "check PageRest upload policy")?;
        let info = PageRestAdapter::parse_info(&info_response.body)?;
        let (enabled, limit, label) = match kind {
            PageRestUploadKind::Book => (true, info.file_limit, "book"),
            PageRestUploadKind::Font => (info.fonts_enabled, info.font_limit, "font"),
            PageRestUploadKind::Image => (info.images_enabled, info.image_limit, "image"),
        };
        if !enabled {
            return Err(SdkError::Unsupported(format!(
                "PageRest {label} upload is currently unavailable"
            )));
        }
        if limit.is_some_and(|limit| limit > 0 && size > limit) {
            return Err(SdkError::InvalidArgument(format!(
                "PageRest {label} exceeds the device's per-file upload limit"
            )));
        }
        if size > info.free_bytes {
            return Err(SdkError::InvalidArgument(
                "PageRest reports insufficient free storage for this upload".to_owned(),
            ));
        }

        let overwrite = match &options.conflict_policy {
            ConflictPolicy::Fail => false,
            ConflictPolicy::OverwriteWhenSupported => true,
            ConflictPolicy::ReplaceWithBackup => {
                return Err(SdkError::Unsupported(
                    "PageRest does not support backup-based replacement".to_owned(),
                ));
            }
        };
        let check_response = self
            .transport
            .execute(PageRestAdapter::upload_check_request(kind, file_name, size))
            .await?;
        let check = PageRestAdapter::parse_upload_check(&check_response)?;
        if check.exists && !overwrite {
            return Err(SdkError::Conflict(format!(
                "PageRest already has a file named {file_name}"
            )));
        }

        let request = PageRestAdapter::upload_request(
            kind,
            local_path,
            file_name,
            overwrite,
            options.content_type.clone(),
        );
        match self
            .transport
            .execute_with_upload_progress(request, progress)
            .await
        {
            Ok(response) => PageRestAdapter::ensure_upload_success(&response),
            Err(upload_error) => {
                if let Ok(response) = self
                    .transport
                    .execute(PageRestAdapter::info_request())
                    .await
                    && PageRestAdapter::ensure_success(&response, "check upload result").is_ok()
                    && let Ok(info) = PageRestAdapter::parse_info(&response.body)
                    && let Some(message) = PageRestAdapter::matching_upload_failure(
                        &info,
                        check.upload_seq,
                        file_name,
                        size,
                    )
                {
                    return Err(SdkError::RemoteFailure(message));
                }
                Err(SdkError::RemoteFailure(format!(
                    "PageRest upload outcome is unconfirmed; check the device before retrying ({upload_error})"
                )))
            }
        }
    }
    async fn execute_success(&self, request: HttpRequest, operation: &str) -> Result<(), SdkError> {
        let response = self.transport.execute(request).await?;
        PageRestAdapter::ensure_mutation_success(&response, operation)
    }
}
#[async_trait]
impl DeviceAdapter for Session {
    async fn verify_connection(&self) -> Result<(), SdkError> {
        let response = self
            .transport
            .execute(PageRestAdapter::info_request())
            .await?;
        PageRestAdapter::ensure_success(&response, "verify device connection")?;
        PageRestAdapter::validate_info(&response.body)
    }
    async fn device_info(&self) -> Result<Vec<DeviceInfoField>, SdkError> {
        let response = self
            .transport
            .execute(PageRestAdapter::info_request())
            .await?;
        PageRestAdapter::ensure_success(&response, "get device info")?;
        PageRestAdapter::device_info(&response.body)
    }
    async fn list_files(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        self.list_pagerest(location.clone()).await
    }
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        let result = upload.result(join_location(&upload.location, &upload.file_name), false);
        self.upload_pagerest(upload, PageRestUploadKind::Book)
            .await?;
        Ok(result)
    }
    fn validate_delete(&self, path: &str) -> Result<(), SdkError> {
        read_pico_name(path).map(|_| ())
    }
    async fn delete(&self, path: &str) -> Result<(), SdkError> {
        self.execute_success(
            PageRestAdapter::delete_request(PageRestUploadKind::Book, read_pico_name(path)?),
            "delete",
        )
        .await
    }
    async fn list_wifi_networks(&self) -> Result<Vec<WifiNetwork>, SdkError> {
        let response = self
            .transport
            .execute(PageRestAdapter::info_request())
            .await?;
        PageRestAdapter::ensure_success(&response, "list Wi-Fi networks")?;
        PageRestAdapter::parse_wifi_info(&response.text_lossy())
    }
    async fn save_wifi_network(&self, credential: &WifiCredential) -> Result<(), SdkError> {
        self.ensure_pagerest_ap_mode().await?;
        self.execute_success(
            PageRestAdapter::wifi_save_request(credential)?,
            "save Wi-Fi network",
        )
        .await
    }
    async fn delete_wifi_network(&self, _index: Option<u32>) -> Result<(), SdkError> {
        if !self.ensure_pagerest_ap_mode().await?.wifi_configured {
            return Err(SdkError::InvalidArgument(
                "PageRest has no saved Wi-Fi network to forget".to_owned(),
            ));
        }
        self.execute_success(
            PageRestAdapter::wifi_delete_request(),
            "delete Wi-Fi network",
        )
        .await
    }
    async fn list_wallpapers(&self) -> Result<Vec<FileEntry>, SdkError> {
        let response = self
            .transport
            .execute(PageRestAdapter::info_request())
            .await?;
        PageRestAdapter::ensure_success(&response, "check PageRest image availability")?;
        if !PageRestAdapter::parse_info(&response.body)?.images_enabled {
            return Err(SdkError::Unsupported(
                "PageRest image management is currently unavailable".to_owned(),
            ));
        }
        let response = self
            .transport
            .execute(PageRestAdapter::files_request("images"))
            .await?;
        PageRestAdapter::ensure_success(&response, "list wallpapers")?;
        PageRestAdapter::parse_media_files(&response.body, "images")
    }
    async fn upload_wallpaper(
        &self,
        upload: WallpaperUpload,
    ) -> Result<WallpaperUploadResult, SdkError> {
        let WallpaperUpload {
            local_path,
            file_name,
            overwrite,
            apply_to_lock_screen,
            size,
        } = upload;
        if apply_to_lock_screen {
            return Err(SdkError::Unsupported(
                    "PageRest uploads images to its wallpaper library but does not apply them from the upload request".to_owned(),
                ));
        }
        let options = UploadOptions {
            conflict_policy: if overwrite {
                ConflictPolicy::OverwriteWhenSupported
            } else {
                ConflictPolicy::Fail
            },
            content_type: None,
            prefer_websocket: false,
        };
        self.upload_pagerest(
            FileUpload {
                local_path,
                file_name: file_name.clone(),
                location: FileLocation::Root,
                options,
                size,
                progress: None,
            },
            PageRestUploadKind::Image,
        )
        .await?;
        Ok(WallpaperUploadResult {
            entry: FileEntry {
                name: file_name.clone(),
                path: format!("/images/{file_name}"),
                size,
                kind: FileKind::Other,
            },
            applied_to_lock_screen: false,
        })
    }
    async fn delete_wallpaper(&self, file_name: &str) -> Result<(), SdkError> {
        let response = self
            .transport
            .execute(PageRestAdapter::info_request())
            .await?;
        PageRestAdapter::ensure_success(&response, "check PageRest image availability")?;
        if !PageRestAdapter::parse_info(&response.body)?.images_enabled {
            return Err(SdkError::Unsupported(
                "PageRest image management is currently unavailable".to_owned(),
            ));
        }
        self.execute_success(
            PageRestAdapter::delete_request(PageRestUploadKind::Image, file_name),
            "delete wallpaper",
        )
        .await
    }
    async fn list_fonts(&self) -> Result<FontCatalog, SdkError> {
        let info_response = self
            .transport
            .execute(PageRestAdapter::info_request())
            .await?;
        PageRestAdapter::ensure_success(&info_response, "check PageRest font availability")?;
        if !PageRestAdapter::parse_info(&info_response.body)?.fonts_enabled {
            return Err(SdkError::Unsupported(
                "PageRest font management is currently unavailable".to_owned(),
            ));
        }
        let response = self
            .transport
            .execute(PageRestAdapter::files_request("fonts"))
            .await?;
        PageRestAdapter::ensure_success(&response, "list fonts")?;
        let files = PageRestAdapter::parse_media_files(&response.body, "fonts")?;
        Ok(FontCatalog {
            max_families: 0,
            families: files
                .into_iter()
                .map(|file| FontFamily {
                    name: file.name.clone(),
                    sizes: Vec::new(),
                    files: vec![FontFile {
                        name: file.name,
                        size: file.size,
                    }],
                })
                .collect(),
        })
    }
    async fn upload_font(&self, upload: FontUpload) -> Result<(), SdkError> {
        let FontUpload {
            local_path,
            file_name,
            overwrite,
            size,
            progress,
            ..
        } = upload;
        let options = UploadOptions {
            conflict_policy: if overwrite {
                ConflictPolicy::OverwriteWhenSupported
            } else {
                ConflictPolicy::Fail
            },
            content_type: None,
            prefer_websocket: false,
        };
        self.upload_pagerest(
            FileUpload {
                local_path,
                file_name,
                location: FileLocation::Root,
                options,
                size,
                progress,
            },
            PageRestUploadKind::Font,
        )
        .await?;
        Ok(())
    }
    async fn delete_font_family(&self, family: &str) -> Result<(), SdkError> {
        validate_file_name(family)?;
        let response = self
            .transport
            .execute(PageRestAdapter::info_request())
            .await?;
        PageRestAdapter::ensure_success(&response, "check PageRest font availability")?;
        if !PageRestAdapter::parse_info(&response.body)?.fonts_enabled {
            return Err(SdkError::Unsupported(
                "PageRest font management is currently unavailable".to_owned(),
            ));
        }
        self.execute_success(
            PageRestAdapter::delete_request(PageRestUploadKind::Font, family),
            "delete font file",
        )
        .await
    }
}
