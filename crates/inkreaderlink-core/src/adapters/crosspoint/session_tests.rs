use crate::model::ConflictPolicy;
use crate::test_support::{MockTransport, response};
use crate::{DeviceClient, DeviceKind, FileLocation, SdkError, SettingChange, UploadOptions};
use std::io::Write;
use tempfile::NamedTempFile;
#[tokio::test]
async fn crosspoint_backup_replace_verifies_before_cleanup() {
    let transport = MockTransport::new(vec![
        response(
            200,
            r#"[{"name":"book.epub","size":3,"isDirectory":false,"isEpub":true}]"#,
        ),
        response(200, "renamed"),
        response(200, "uploaded"),
        response(
            200,
            r#"[{"name":"book.epub","size":4,"isDirectory":false,"isEpub":true},{"name":"book.epub.back","size":3,"isDirectory":false,"isEpub":false}]"#,
        ),
        response(200, "deleted"),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::CrossPoint, transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();

    let result = client
        .upload(
            file.path().to_path_buf(),
            "book.epub".to_owned(),
            FileLocation::Directory("/Books".to_owned()),
            UploadOptions {
                conflict_policy: ConflictPolicy::ReplaceWithBackup,
                content_type: None,
                prefer_websocket: false,
            },
            None,
        )
        .await
        .unwrap();

    assert_eq!(result.entry.path, "/Books/book.epub");
    assert_eq!(transport.request_count(), 5);
}

#[tokio::test]
async fn settings_reject_stale_snapshot_before_post() {
    let old = r#"[{"key":"fontSize","name":"Font size","category":"Reader","type":"enum","value":0,"options":["12 pt","14 pt"]}]"#;
    let changed = r#"[{"key":"fontSize","name":"Font size","category":"Reader","type":"enum","value":0,"options":["10 pt","12 pt"]}]"#;
    let transport = MockTransport::new(vec![response(200, old), response(200, changed)]);
    let client = DeviceClient::with_transport(DeviceKind::CrossPoint, transport.clone());
    let snapshot = client.list_settings().await.unwrap();
    let result = client
        .apply_settings(
            snapshot,
            vec![SettingChange {
                key: "fontSize".to_owned(),
                value: crate::SettingValue::Choice(1),
            }],
        )
        .await;
    assert!(matches!(result, Err(SdkError::Conflict(_))));
    assert_eq!(transport.request_count(), 2);
}

#[tokio::test]
async fn settings_post_only_changed_values_and_return_refreshed_snapshot() {
    let old = r#"[{"key":"showHiddenFiles","name":"Show hidden files","category":"Files","type":"toggle","value":0}]"#;
    let new = r#"[{"key":"showHiddenFiles","name":"Show hidden files","category":"Files","type":"toggle","value":1}]"#;
    let transport = MockTransport::new(vec![
        response(200, old),
        response(200, old),
        response(200, "Applied 1 setting(s)"),
        response(200, new),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::CrossPoint, transport.clone());
    let snapshot = client.list_settings().await.unwrap();
    let updated = client
        .apply_settings(
            snapshot,
            vec![SettingChange {
                key: "showHiddenFiles".to_owned(),
                value: crate::SettingValue::Toggle(true),
            }],
        )
        .await
        .unwrap();
    assert_eq!(updated.settings[0].value, crate::SettingValue::Toggle(true));
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].path, "/api/settings");
    assert!(
        matches!(&requests[2].body, crate::transport::HttpBody::Json(body)
        if serde_json::from_slice::<serde_json::Value>(body).unwrap() == serde_json::json!({"showHiddenFiles":1}))
    );
}

#[tokio::test]
async fn failed_replacement_restores_backup_or_reports_recovery_failure() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();
    for restore_status in [200, 500] {
        let transport = MockTransport::new(vec![
            response(
                200,
                r#"[{"name":"book.epub","size":3,"isDirectory":false,"isEpub":true}]"#,
            ),
            response(200, "renamed"),
            response(500, "upload failed"),
            response(restore_status, "restore"),
        ]);
        let client = DeviceClient::with_transport(DeviceKind::CrossPoint, transport.clone());
        let error = client
            .upload(
                file.path().to_path_buf(),
                "book.epub".to_owned(),
                FileLocation::Root,
                UploadOptions {
                    conflict_policy: ConflictPolicy::ReplaceWithBackup,
                    content_type: None,
                    prefer_websocket: false,
                },
                None,
            )
            .await
            .unwrap_err();
        if restore_status == 200 {
            assert!(matches!(error, SdkError::RemoteFailure(_)));
        } else {
            assert!(matches!(error, SdkError::RecoveryFailed(_)));
        }
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[3].path, "/rename");
    }
}
