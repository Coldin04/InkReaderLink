//! Stateful `PicoPageAdapter` protocol operations.
use crate::adapters::{DeviceAdapter, FileUpload};
use crate::model::ConflictPolicy;
use crate::transport::SharedTransport;
use crate::validation::join_location;
use crate::{FileLocation, SdkError, UploadOptions, UploadResult};
use async_trait::async_trait;
use std::{collections::HashMap, path::PathBuf};

use super::ACCESS_CODE_KEY;
use super::PicoPageAdapter;
pub(crate) struct Session {
    transport: SharedTransport,
    origin: Option<String>,
    access_code: Option<String>,
}
impl Session {
    pub fn new(transport: SharedTransport) -> Self {
        Self {
            origin: transport.origin(),
            transport,
            access_code: None,
        }
    }
}
impl Session {
    async fn upload_pico_page(
        &self,
        local_path: PathBuf,
        file_name: &str,
        _location: &FileLocation,
        options: UploadOptions,
    ) -> Result<(), SdkError> {
        if options.conflict_policy != ConflictPolicy::Fail {
            return Err(SdkError::Unsupported(
                "Pico Page does not support overwriting files".to_owned(),
            ));
        }
        let access_code =
            self.pico_page_access_code()
                .ok_or_else(|| SdkError::CredentialRequired {
                    key: ACCESS_CODE_KEY.to_owned(),
                })?;

        let page = self
            .transport
            .execute(PicoPageAdapter::page_request(access_code)?)
            .await?;
        let session_token = PicoPageAdapter::validate_page(&page, access_code)?;
        let (size, crc32) = PicoPageAdapter::file_checksum(&local_path).await?;
        if size == 0 {
            return Err(SdkError::InvalidArgument(
                "upload file must not be empty".to_owned(),
            ));
        }
        let check = self
            .transport
            .execute(PicoPageAdapter::check_request(
                file_name,
                size,
                &crc32,
                &session_token,
                self.origin.as_deref(),
                access_code,
            ))
            .await?;
        PicoPageAdapter::ensure_success(&check, "check upload")?;

        let response = self
            .transport
            .execute_with_upload_progress(
                PicoPageAdapter::upload_request(
                    local_path,
                    file_name,
                    size,
                    &crc32,
                    &session_token,
                    self.origin.as_deref(),
                    access_code,
                ),
                None,
            )
            .await?;
        PicoPageAdapter::ensure_success(&response, "upload")
    }
    fn pico_page_access_code(&self) -> Option<&str> {
        self.access_code.as_deref()
    }
}
#[async_trait]
impl DeviceAdapter for Session {
    fn configure_address(&mut self, address: &str) -> Result<(), SdkError> {
        let (origin, code) = PicoPageAdapter::connection_origin(address)?;
        self.origin = Some(origin);
        self.access_code = code;
        Ok(())
    }

    async fn verify_connection(&self) -> Result<(), SdkError> {
        let access_code =
            self.pico_page_access_code()
                .ok_or_else(|| SdkError::CredentialRequired {
                    key: ACCESS_CODE_KEY.to_owned(),
                })?;
        let response = self
            .transport
            .execute(PicoPageAdapter::page_request(access_code)?)
            .await?;
        PicoPageAdapter::validate_page(&response, access_code)?;
        Ok(())
    }
    fn set_session_credentials(&mut self, credentials: HashMap<String, String>) {
        if let Some(code) = credentials.get(ACCESS_CODE_KEY) {
            self.access_code = Some(code.clone());
        }
    }
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        let path = join_location(&upload.location, &upload.file_name);
        self.upload_pico_page(
            upload.local_path.clone(),
            &upload.file_name,
            &upload.location,
            upload.options.clone(),
        )
        .await?;
        Ok(upload.result(path, false))
    }
}
