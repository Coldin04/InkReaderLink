use super::ACCESS_CODE_KEY;
use crate::test_support::{MockTransport, RecordingProgress, response};
use crate::{DeviceClient, DeviceKind, FileLocation, SdkError, UploadOptions};
use std::path::PathBuf;
use std::{collections::HashMap, io::Write, sync::Arc};
use tempfile::NamedTempFile;
#[tokio::test]
async fn justread_probe_requests_code_and_rejects_wrong_code_without_leaking_it() {
    let transport = MockTransport::new(vec![response(401, "访问码错误")]);
    let client = DeviceClient::with_transport(DeviceKind::JustRead, transport.clone());
    assert_eq!(
        client.verify_connection().await,
        Err(SdkError::CredentialRequired {
            key: ACCESS_CODE_KEY.to_owned(),
        })
    );
    assert!(
        !transport.requests.lock().unwrap()[0]
            .headers
            .iter()
            .any(|(name, _)| name == "X-Transfer-Token")
    );

    let transport = MockTransport::new(vec![response(401, "访问码错误")]);
    let client = DeviceClient::with_transport(DeviceKind::JustRead, transport.clone())
        .with_session_credentials(HashMap::from([(
            ACCESS_CODE_KEY.to_owned(),
            "WRONG".to_owned(),
        )]));
    assert_eq!(
        client.verify_connection().await,
        Err(SdkError::CredentialRejected {
            key: ACCESS_CODE_KEY.to_owned(),
        })
    );
    assert!(!format!("{client:?}").contains("WRONG"));
}

#[tokio::test]
async fn justread_session_uses_code_for_listing_and_uploads_books_to_shelf() {
    let empty = r#"{"entries":[],"truncated":false}"#;
    let transport = MockTransport::new(vec![
        response(200, empty),
        response(200, empty),
        response(200, r#"{"ok":true}"#),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::JustRead, transport.clone())
        .with_session_credentials(HashMap::from([(
            ACCESS_CODE_KEY.to_owned(),
            "ABCD2345EFGH".to_owned(),
        )]));
    client.verify_connection().await.unwrap();
    client.list_files(FileLocation::Root).await.unwrap();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();
    let result = client
        .upload(
            file.path().to_path_buf(),
            "book.epub".to_owned(),
            FileLocation::Root,
            UploadOptions::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.entry.path, "/books/book.epub");
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|request| {
        request
            .headers
            .iter()
            .any(|(name, value)| name == "X-Transfer-Token" && value == "ABCD2345EFGH")
    }));
    assert_eq!(requests[2].path, "/api/upload");
    assert!(
        requests[2]
            .headers
            .iter()
            .any(|(name, value)| { name == "X-Transfer-Path" && value == "Ym9va3MvYm9vay5lcHVi" })
    );
}

#[tokio::test]
async fn justread_upload_forwards_http_byte_progress() {
    let transport = MockTransport::new(vec![response(200, r#"{"ok":true}"#)]);
    let client = DeviceClient::with_transport(DeviceKind::JustRead, transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();
    let progress = Arc::new(RecordingProgress::default());

    client
        .upload(
            file.path().to_path_buf(),
            "book.epub".to_owned(),
            FileLocation::Root,
            UploadOptions::default(),
            Some(progress.clone()),
        )
        .await
        .unwrap();

    assert_eq!(*transport.upload_progress_observers.lock().unwrap(), [true]);
    assert_eq!(*progress.0.lock().unwrap(), [(2, 4)]);
}

#[tokio::test]
async fn justread_download_exchanges_authenticated_ticket_for_streaming_request() {
    let transport = MockTransport::new(vec![response(
        200,
        r#"{"url":"/api/download-file?t=ABCD2345EFGH","name":"book.epub"}"#,
    )]);
    let client = DeviceClient::with_transport(DeviceKind::JustRead, transport.clone())
        .with_session_credentials(HashMap::from([(
            ACCESS_CODE_KEY.to_owned(),
            "SESSIONCODE12".to_owned(),
        )]));
    client
        .download("/books/book.epub".to_owned(), PathBuf::from("unused"))
        .await
        .unwrap();
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests[0].path, "/api/download-ticket");
    assert!(
        requests[0]
            .headers
            .iter()
            .any(|(name, value)| { name == "X-Transfer-Token" && value == "SESSIONCODE12" })
    );
    let downloads = transport.downloads.lock().unwrap();
    assert_eq!(downloads[0].path, "/api/download-file");
    assert_eq!(
        downloads[0].query,
        [("t".to_owned(), "ABCD2345EFGH".to_owned())]
    );
}
