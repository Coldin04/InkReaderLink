use async_trait::async_trait;

use crate::{
    FileLocation, SdkError, UploadResult,
    adapters::{DeviceAdapter, FileUpload},
    model::ConflictPolicy,
    transport::SharedTransport,
    validation::join_location,
};

use super::FolioBookAdapter;

pub(crate) struct Session {
    transport: SharedTransport,
}

impl Session {
    pub fn new(transport: SharedTransport) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl DeviceAdapter for Session {
    async fn verify_connection(&self) -> Result<(), SdkError> {
        let response = self
            .transport
            .execute(FolioBookAdapter::page_request())
            .await?;
        FolioBookAdapter::validate_page(&response)
    }

    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        if !matches!(upload.location, FileLocation::Root) {
            return Err(SdkError::Unsupported(
                "FolioBook does not support directory selection".to_owned(),
            ));
        }
        if upload.options.conflict_policy != ConflictPolicy::Fail || upload.options.prefer_websocket
        {
            return Err(SdkError::Unsupported(
                "FolioBook does not support explicit replacement or WebSocket uploads".to_owned(),
            ));
        }
        let response = self
            .transport
            .execute_with_upload_progress(
                FolioBookAdapter::upload_request(
                    upload.local_path.clone(),
                    &upload.file_name,
                    upload.options.content_type.clone(),
                ),
                upload.progress.clone(),
            )
            .await?;
        FolioBookAdapter::ensure_success(&response, "upload")?;
        Ok(upload.result(join_location(&upload.location, &upload.file_name), false))
    }
}
