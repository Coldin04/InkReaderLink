use crate::{
    DeviceClient, DeviceKind, FileLocation, SdkError, UploadOptions, adapters,
    test_support::{MockTransport, RecordingProgress, response},
};
use std::{collections::HashMap, io::Write, sync::Arc};
use tempfile::NamedTempFile;
const PAGE: &str =
    r#"<input id="files"><div id="list"></div><script>const token = 'sessiontoken';</script>"#;
#[tokio::test]
async fn connection_address_is_interpreted_only_by_the_adapter() {
    let transport = MockTransport::new(vec![response(200, PAGE)]);
    let mut adapter = adapters::create(DeviceKind::PicoPage, transport.clone());
    adapter
        .configure_address("http://reader.example.test/CODE123")
        .unwrap();
    adapter.verify_connection().await.unwrap();
    assert_eq!(transport.requests.lock().unwrap()[0].path, "/CODE123");
    assert!(
        adapter
            .configure_address("http://reader.example.test/code/extra")
            .is_err()
    );
}
#[tokio::test]
async fn upload_runs_the_complete_handshake_with_session_headers() {
    let transport = MockTransport::new(vec![
        response(200, PAGE),
        response(200, "ok"),
        response(200, "ok"),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::PicoPage, transport.clone())
        .with_session_credentials(HashMap::from([(
            "access_code".to_owned(),
            "CODE123".to_owned(),
        )]));
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"123456789").unwrap();
    let progress = Arc::new(RecordingProgress::default());
    let result = client
        .upload(
            file.path().to_path_buf(),
            "中文 book.epub".to_owned(),
            FileLocation::Root,
            UploadOptions::default(),
            Some(progress.clone()),
        )
        .await
        .unwrap();
    assert_eq!(result.entry.size, 9);
    assert!(!result.used_websocket);
    let requests = transport.requests.lock().unwrap();
    assert_eq!(
        requests.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
        ["/CODE123", "/check", "/upload"]
    );
    for request in &requests[1..] {
        for (key, value) in [
            ("X-Pico-CRC", "cbf43926"),
            ("X-Pico-Token", "sessiontoken"),
            ("Origin", "http://reader.example.test"),
            ("Referer", "http://reader.example.test/CODE123"),
        ] {
            assert!(request.headers.iter().any(|(k, v)| k == key && v == value));
        }
        assert!(!format!("{request:?}").contains("sessiontoken"));
    }
    assert_eq!(
        *transport.upload_progress_observers.lock().unwrap(),
        [false]
    );
    assert!(progress.0.lock().unwrap().is_empty());
}
#[tokio::test]
async fn sessions_do_not_share_codes_and_missing_or_rejected_codes_are_typed() {
    let transport = MockTransport::new(vec![response(404, "")]);
    let first = DeviceClient::with_transport(DeviceKind::PicoPage, transport.clone())
        .with_session_credentials(HashMap::from([(
            "access_code".to_owned(),
            "CODE123".to_owned(),
        )]));
    let second = DeviceClient::with_transport(DeviceKind::PicoPage, transport.clone());
    assert_eq!(
        second.verify_connection().await,
        Err(SdkError::CredentialRequired {
            key: "access_code".to_owned()
        })
    );
    assert_eq!(transport.request_count(), 0);
    assert_eq!(
        first.verify_connection().await,
        Err(SdkError::CredentialRejected {
            key: "access_code".to_owned()
        })
    );
    assert!(!format!("{first:?}").contains("CODE123"));
}
#[tokio::test]
async fn supplied_credentials_override_the_code_from_the_address() {
    let transport = MockTransport::new(vec![response(200, PAGE)]);
    let mut adapter = adapters::create(DeviceKind::PicoPage, transport.clone());
    adapter
        .configure_address("http://reader.example.test/OLD")
        .unwrap();
    adapter.set_session_credentials(HashMap::from([(
        "access_code".to_owned(),
        "NEW".to_owned(),
    )]));
    adapter.verify_connection().await.unwrap();
    assert_eq!(transport.requests.lock().unwrap()[0].path, "/NEW");
}
