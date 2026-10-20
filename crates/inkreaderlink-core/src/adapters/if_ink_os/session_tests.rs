use crate::{
    DeviceClient, DeviceKind, FileLocation, SdkError, UploadOptions,
    config::if_ink_os_definition,
    route_device,
    test_support::{MockTransport, response},
};
use std::io::Write;
use tempfile::NamedTempFile;
#[tokio::test]
async fn adapter_selection_survives_profile_identity_changes() {
    assert_eq!(route_device("if-ink-os"), Some(DeviceKind::IfInkOs));
    let transport = MockTransport::new(vec![response(200, "ok")]);
    let mut definition = if_ink_os_definition();
    definition.device_type = "renamed-reader".to_owned();
    let client = DeviceClient::with_transport(DeviceKind::IfInkOs, transport.clone())
        .with_device_definition(definition);
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();
    client
        .upload(
            file.path().to_path_buf(),
            "book.md".to_owned(),
            FileLocation::Directory("/markdown".to_owned()),
            UploadOptions::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        transport.requests.lock().unwrap()[0].path,
        "/document-upload"
    );
}
#[tokio::test]
async fn metadata_cannot_switch_a_different_adapter_to_this_protocol() {
    let transport = MockTransport::new(vec![response(200, "ok")]);
    let client = DeviceClient::with_transport(DeviceKind::KiikoRead, transport.clone())
        .with_device_definition(if_ink_os_definition());
    assert!(matches!(
        client
            .rename("/book.epub".to_owned(), "new.epub".to_owned())
            .await,
        Err(SdkError::Unsupported(_))
    ));
    assert_eq!(transport.request_count(), 0);
}
#[tokio::test]
async fn incompatible_markdown_destination_is_rejected_before_io() {
    let transport = MockTransport::new(vec![]);
    let client = DeviceClient::with_transport(DeviceKind::IfInkOs, transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();
    assert!(matches!(
        client
            .upload(
                file.path().to_path_buf(),
                "book.md".to_owned(),
                FileLocation::Directory("/markdown/sub".to_owned()),
                UploadOptions::default(),
                None
            )
            .await,
        Err(SdkError::Unsupported(_))
    ));
    assert_eq!(transport.request_count(), 0);
}
