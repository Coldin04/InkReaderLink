use crate::{
    DeviceClient, DeviceKind, FileLocation, SdkError, UploadOptions, WifiCredential,
    test_support::{MockTransport, response},
    transport::{HttpBody, HttpMethod},
};
use std::io::Write;
use tempfile::NamedTempFile;

const INFO: &str = r#"{"mode":"ap","free_bytes":10000,"is_flash":false,"file_limit":5000,"wifi_configured":true,"wifi_ssid":"test-net","fonts_enabled":true,"font_root":"/fonts","font_limit":2000,"storage_root":"/library","book_px":56,"book_bold":false,"reader_heading_right":false,"reader_margin":1,"reader_line_permille":1650,"reader_underline":0,"reader_wave":2,"reader_full_every":10,"reader_indent":true,"reader_justify":false,"reader_chapter_opt":true,"reader_blank_opt":false}"#;

#[tokio::test]
async fn book_upload_is_raw_sequential_and_checks_conflicts() {
    let transport = MockTransport::new(vec![
        response(200, INFO),
        response(404, "{}"),
        response(200, r#"{"ok":true}"#),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::AuraReader, transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"synthetic book").unwrap();
    client
        .upload(
            file.path().to_path_buf(),
            "sample.epub".into(),
            FileLocation::Root,
            UploadOptions::default(),
            None,
        )
        .await
        .unwrap();
    let req = transport.requests.lock().unwrap();
    assert_eq!(
        req.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
        ["/info", "/books", "/upload"]
    );
    assert_eq!(req[2].method, HttpMethod::Put);
    assert!(matches!(req[2].body, HttpBody::RawFile { .. }));
    assert!(
        req[2]
            .query
            .iter()
            .any(|(k, v)| k == "name" && v == "sample.epub")
    );
}

#[tokio::test]
async fn wifi_mutation_is_blocked_in_station_mode() {
    let transport = MockTransport::new(vec![response(200, &INFO.replace("\"ap\"", "\"sta\""))]);
    let client = DeviceClient::with_transport(DeviceKind::AuraReader, transport.clone());
    assert!(matches!(
        client
            .save_wifi_network(WifiCredential {
                index: None,
                ssid: "test".into(),
                password: None
            })
            .await,
        Err(SdkError::Unsupported(_))
    ));
    assert_eq!(transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn settings_are_read_from_info_and_saved_via_system() {
    let transport = MockTransport::new(vec![
        response(200, INFO),
        response(200, INFO),
        response(200, r#"{"ok":true}"#),
        response(200, INFO),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::AuraReader, transport.clone());
    let before = client.list_settings().await.unwrap();
    let after = client
        .apply_settings(
            before,
            vec![crate::SettingChange {
                key: "book_px".into(),
                value: crate::SettingValue::Number(58),
            }],
        )
        .await
        .unwrap();
    assert_eq!(
        after
            .settings
            .iter()
            .find(|s| s.key == "book_px")
            .unwrap()
            .value,
        crate::SettingValue::Number(56)
    );
    let req = transport.requests.lock().unwrap();
    assert_eq!(req[2].path, "/system");
    assert_eq!(req[2].method, HttpMethod::Post);
}

#[tokio::test]
async fn committed_progress_cleanup_failure_is_preserved_even_with_error_status() {
    let transport = MockTransport::new(vec![
        response(200, INFO),
        response(404, r#"{"error":"missing"}"#),
        response(
            500,
            r#"{"progress_cleanup_failed":true,"error":"cleanup failed"}"#,
        ),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::AuraReader, transport);
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"synthetic book").unwrap();
    assert!(matches!(
        client
            .upload(
                file.path().to_path_buf(),
                "sample.epub".into(),
                FileLocation::Root,
                UploadOptions::default(),
                None,
            )
            .await,
        Err(SdkError::CommittedButCleanupFailed(message)) if message == "cleanup failed"
    ));
}

#[tokio::test]
async fn book_listing_uses_server_root_and_reads_all_pages() {
    let transport = MockTransport::new(vec![
        response(
            200,
            r#"{"root":"/reader/library","root_label":"test","page":0,"total":2,"pages":2,"items":[{"name":"one.epub","size":10}]}"#,
        ),
        response(
            200,
            r#"{"root":"/reader/library","root_label":"test","page":1,"total":2,"pages":2,"items":[{"name":"two.txt","size":20}]}"#,
        ),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::AuraReader, transport.clone());
    let entries = client.list_files(FileLocation::Root).await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].path, "/reader/library/one.epub");
    assert_eq!(entries[1].path, "/reader/library/two.txt");
    assert_eq!(transport.requests.lock().unwrap().len(), 2);
}
