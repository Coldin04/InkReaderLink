use crate::test_support::{MockTransport, response};
use crate::transport::HttpMethod;
use crate::{DeviceClient, DeviceKind, FileLocation, SdkError};
use std::io::Write;
use tempfile::NamedTempFile;
#[tokio::test]
async fn read_pico_font_upload_uses_font_endpoint_and_preserves_conflict() {
    let transport = MockTransport::new(vec![
        response(200, r#"{"name":"test.ttf","size":9}"#),
        response(200, r#"{"ok":true}"#),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"font data").unwrap();
    let path = file.path().to_path_buf();
    let first = client
        .upload_font(String::new(), path.clone(), "test.ttf".to_owned())
        .await;
    assert!(matches!(first, Err(SdkError::Conflict(_))));
    client
        .upload_font_with_overwrite(String::new(), path, "test.ttf".to_owned(), true)
        .await
        .unwrap();
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests[0].method, HttpMethod::Get);
    assert_eq!(requests[0].path, "/fonts");
    assert!(
        requests[0]
            .query
            .iter()
            .any(|(key, value)| key == "name" && value == "test.ttf")
    );
    assert_eq!(requests[1].method, HttpMethod::Put);
    assert_eq!(requests[1].path, "/fonts");
    assert!(
        requests[1]
            .query
            .iter()
            .any(|(key, value)| key == "overwrite" && value == "1")
    );
}

#[tokio::test]
async fn read_pico_font_upload_reports_missing_card() {
    let transport = MockTransport::new(vec![response(409, r#"{"error":"字体需要 TF 卡"}"#)]);
    let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport);
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"font data").unwrap();
    let result = client
        .upload_font(
            String::new(),
            file.path().to_path_buf(),
            "test.ttf".to_owned(),
        )
        .await;
    assert!(matches!(result, Err(SdkError::Unsupported(_))));
}

#[tokio::test]
async fn read_pico_file_pages_are_hidden_from_callers() {
    let transport = MockTransport::new(vec![
        response(
            200,
            r#"{"page":0,"total":2,"pages":2,"items":[{"name":"a.epub","size":1}]}"#,
        ),
        response(
            200,
            r#"{"page":1,"total":2,"pages":2,"items":[{"name":"b.txt","size":2}]}"#,
        ),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport.clone());

    let files = client.list_files(FileLocation::Root).await.unwrap();

    assert_eq!(files.len(), 2);
    assert_eq!(transport.request_count(), 2);
}
