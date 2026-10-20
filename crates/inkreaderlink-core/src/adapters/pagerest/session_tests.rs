use crate::{
    DeviceClient, DeviceKind, FileLocation, SdkError, UploadOptions, WifiCredential,
    test_support::{MockTransport, response},
    transport::HttpMethod,
};
use std::io::Write;
use tempfile::NamedTempFile;
const INFO: &str = r#"{"is_flash":false,"free_bytes":1000,"mode":"ap","wifi_configured":true,"fonts_enabled":true,"images_enabled":true}"#;
#[tokio::test]
async fn upload_uses_preflight_and_raw_body_without_browser_authentication() {
    let transport = MockTransport::new(vec![
        response(200, INFO),
        response(200, r#"{"exists":false,"upload_seq":1}"#),
        response(200, r#"{"ok":true}"#),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::PageRest, transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();
    client
        .upload(
            file.path().to_path_buf(),
            "book.epub".to_owned(),
            FileLocation::Root,
            UploadOptions::default(),
            None,
        )
        .await
        .unwrap();
    let requests = transport.requests.lock().unwrap();
    assert_eq!(
        requests.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
        ["/info", "/upload-check", "/upload"]
    );
    assert!(
        !requests
            .iter()
            .flat_map(|r| r.headers.iter())
            .any(|(k, _)| k == "X-Pico-Token")
    );
}
#[tokio::test]
async fn font_file_deletion_checks_availability_and_uses_media_kind() {
    let transport = MockTransport::new(vec![response(200, INFO), response(200, r#"{"ok":true}"#)]);
    let client = DeviceClient::with_transport(DeviceKind::PageRest, transport.clone());
    client
        .delete_font_family("font.ttf".to_owned())
        .await
        .unwrap();
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].method, HttpMethod::Delete);
    assert_eq!(requests[1].path, "/fonts");
    assert_eq!(
        requests[1].query,
        [("name".to_owned(), "font.ttf".to_owned())]
    );
}
#[tokio::test]
async fn wifi_changes_are_rejected_in_station_mode_before_mutation() {
    let transport = MockTransport::new(vec![response(200, &INFO.replace("ap", "sta"))]);
    let client = DeviceClient::with_transport(DeviceKind::PageRest, transport.clone());
    assert!(matches!(
        client
            .save_wifi_network(WifiCredential {
                index: None,
                ssid: "test".to_owned(),
                password: None
            })
            .await,
        Err(SdkError::Unsupported(_))
    ));
    assert_eq!(transport.request_count(), 1);
    assert_eq!(
        transport.requests.lock().unwrap()[0].method,
        HttpMethod::Get
    );
}
#[tokio::test]
async fn existing_upload_is_reported_as_conflict_without_sending_file() {
    let transport = MockTransport::new(vec![
        response(200, INFO),
        response(200, r#"{"exists":true}"#),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::PageRest, transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();
    assert!(matches!(
        client
            .upload(
                file.path().to_path_buf(),
                "book.epub".to_owned(),
                FileLocation::Root,
                UploadOptions::default(),
                None
            )
            .await,
        Err(SdkError::Conflict(_))
    ));
    assert_eq!(transport.request_count(), 2);
}
