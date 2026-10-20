use crate::{
    DeviceClient, DeviceKind, FileLocation, SdkError, UploadOptions,
    test_support::{MockTransport, response},
};
use std::io::Write;
use tempfile::NamedTempFile;
#[tokio::test]
async fn upload_preserves_storage_error_mapping() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();
    for status in [413, 507] {
        let transport = MockTransport::new(vec![response(status, "full")]);
        let client = DeviceClient::with_transport(DeviceKind::KiikoRead, transport);
        assert_eq!(
            client
                .upload(
                    file.path().to_path_buf(),
                    "book.epub".to_owned(),
                    FileLocation::Root,
                    UploadOptions::default(),
                    None
                )
                .await,
            Err(SdkError::InsufficientStorage)
        );
    }
}
