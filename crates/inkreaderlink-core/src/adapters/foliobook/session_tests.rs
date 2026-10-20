use std::{collections::HashMap, io::Write, sync::Arc, time::Duration};

use tempfile::NamedTempFile;

use crate::{
    Capability, ConflictPolicy, DeviceClient, DeviceKind, FileLocation, SdkError, UploadOptions,
    built_in_definition, built_in_definitions,
    capability::ids,
    route_device,
    test_support::{MockTransport, RecordingProgress, response},
    transport::{HttpBody, HttpMethod},
};

const PAGE: &str = "<!DOCTYPE html><title>Folio Book</title><script>
function send(file){var x=new XMLHttpRequest();
x.open('POST','/up?name='+encodeURIComponent(file.name));x.send(file);}
</script>";

#[test]
fn registered_device_exposes_only_confirmed_upload_capabilities() {
    assert_eq!(route_device("foliobook"), Some(DeviceKind::FolioBook));
    let definition = built_in_definition(DeviceKind::FolioBook);
    assert!(built_in_definitions().contains(&definition));
    assert_eq!(definition.display_name, "FolioBook");
    assert_eq!(definition.capabilities, [Capability::new(ids::FILE_UPLOAD)]);
    assert!(!definition.constraints.can_list_directories);
    assert!(!definition.constraints.can_choose_upload_directory);
    assert!(definition.file_formats.accepts_any_upload_format);
    assert_eq!(
        definition.file_formats.readable_extensions,
        Vec::<String>::new()
    );
    assert_eq!(
        definition.file_formats.font_upload_extensions,
        Vec::<String>::new()
    );
    assert_eq!(
        definition.file_formats.wallpaper_upload_extensions,
        Vec::<String>::new()
    );
    assert_eq!(definition.connection_fields.len(), 1);
    assert_eq!(definition.connection_fields[0].key, "address");
}

#[tokio::test]
async fn connection_probe_requires_the_uploader_and_never_writes() {
    for (status, body, valid) in [
        (200, PAGE, true),
        (200, "<title>Folio Book</title>", false),
        (
            200,
            "<title>Other reader</title>/up?name=encodeURIComponent",
            false,
        ),
        (403, PAGE, false),
    ] {
        let transport = MockTransport::new(vec![response(status, body)]);
        let client = DeviceClient::with_transport(DeviceKind::FolioBook, transport.clone());
        assert_eq!(client.verify_connection().await.is_ok(), valid);
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, HttpMethod::Get);
        assert_eq!(requests[0].path, "/");
        assert_eq!(requests[0].body, HttpBody::Empty);
    }
}

#[tokio::test]
async fn upload_sends_one_raw_file_and_forwards_progress_without_session_secrets() {
    let transport = MockTransport::new(vec![response(200, "saved")]);
    // Changing the visible profile and supplying another firmware's credential
    // must not select that firmware's protocol or add authentication headers.
    let client = DeviceClient::with_transport(DeviceKind::FolioBook, transport.clone())
        .with_device_definition(built_in_definition(DeviceKind::CrossPoint))
        .with_session_credentials(HashMap::from([(
            "access_code".to_owned(),
            "secret".to_owned(),
        )]));
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"data").unwrap();
    let progress = Arc::new(RecordingProgress::default());
    let name = "样例 +&?#%.pdf";
    let result = client
        .upload(
            file.path().to_path_buf(),
            name.to_owned(),
            FileLocation::Root,
            UploadOptions {
                content_type: Some("application/pdf".to_owned()),
                ..UploadOptions::default()
            },
            Some(progress.clone()),
        )
        .await
        .unwrap();
    assert_eq!(result.entry.path, format!("/{name}"));
    assert_eq!(result.entry.size, 4);
    assert!(!result.used_websocket);
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, HttpMethod::Post);
    assert_eq!(requests[0].path, "/up");
    assert_eq!(requests[0].query, [("name".to_owned(), name.to_owned())]);
    assert_eq!(requests[0].headers, Vec::<(String, String)>::new());
    assert_eq!(
        requests[0].body,
        HttpBody::RawFile {
            path: file.path().to_path_buf(),
            content_type: Some("application/pdf".to_owned()),
        }
    );
    assert_eq!(*progress.0.lock().unwrap(), [(2, 4)]);
}

#[tokio::test]
async fn unsupported_operations_and_upload_modes_do_not_reach_the_device() {
    let transport = MockTransport::new(vec![]);
    let client = DeviceClient::with_transport(DeviceKind::FolioBook, transport.clone());
    assert!(matches!(
        client.list_files(FileLocation::Root).await,
        Err(SdkError::Unsupported(_))
    ));
    assert!(matches!(
        client.delete("/sample.pdf".to_owned()).await,
        Err(SdkError::Unsupported(_))
    ));
    assert!(matches!(
        client.device_info().await,
        Err(SdkError::Unsupported(_))
    ));
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"data").unwrap();
    for (location, options) in [
        (
            FileLocation::Directory("/books".to_owned()),
            UploadOptions::default(),
        ),
        (
            FileLocation::Root,
            UploadOptions {
                conflict_policy: ConflictPolicy::OverwriteWhenSupported,
                ..UploadOptions::default()
            },
        ),
        (
            FileLocation::Root,
            UploadOptions {
                conflict_policy: ConflictPolicy::ReplaceWithBackup,
                ..UploadOptions::default()
            },
        ),
        (
            FileLocation::Root,
            UploadOptions {
                prefer_websocket: true,
                ..UploadOptions::default()
            },
        ),
    ] {
        assert!(matches!(
            client
                .upload(
                    file.path().to_path_buf(),
                    "sample.pdf".to_owned(),
                    location,
                    options,
                    None
                )
                .await,
            Err(SdkError::Unsupported(_))
        ));
    }
    assert_eq!(transport.request_count(), 0);
    // Even an expanded profile cannot make this adapter silently ignore a mode.
    let client = client.with_device_definition(built_in_definition(DeviceKind::CrossPoint));
    assert!(matches!(
        client
            .upload(
                file.path().to_path_buf(),
                "sample.pdf".to_owned(),
                FileLocation::Directory("/books".to_owned()),
                UploadOptions::default(),
                None
            )
            .await,
        Err(SdkError::Unsupported(_))
    ));
    assert_eq!(transport.request_count(), 0);
}

#[tokio::test]
async fn only_http_200_succeeds_and_a_failed_upload_does_not_poison_the_session() {
    let transport = MockTransport::new(vec![
        response(201, "unexpected status"),
        response(409, "file exists"),
        response(500, "storage failure"),
        response(200, ""),
    ]);
    let client = DeviceClient::with_transport(DeviceKind::FolioBook, transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"data").unwrap();
    for (status, message) in [
        (201, "unexpected status"),
        (409, "file exists"),
        (500, "storage failure"),
    ] {
        let result = client
            .upload(
                file.path().to_path_buf(),
                "sample".to_owned(),
                FileLocation::Root,
                UploadOptions::default(),
                None,
            )
            .await;
        assert!(
            matches!(result, Err(SdkError::RemoteFailure(detail)) if detail.contains(&status.to_string()) && detail.contains(message))
        );
    }
    client
        .upload(
            file.path().to_path_buf(),
            "sample".to_owned(),
            FileLocation::Root,
            UploadOptions::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(transport.request_count(), 4);
}

#[tokio::test]
async fn clients_keep_their_transport_and_failure_state_isolated() {
    let first_transport = MockTransport::new(vec![response(500, "storage failure")]);
    let second_transport = MockTransport::new(vec![response(200, "saved")]);
    let first = DeviceClient::with_transport(DeviceKind::FolioBook, first_transport.clone());
    let second = DeviceClient::with_transport(DeviceKind::FolioBook, second_transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"data").unwrap();
    assert!(
        first
            .upload(
                file.path().to_path_buf(),
                "first.bin".to_owned(),
                FileLocation::Root,
                UploadOptions::default(),
                None
            )
            .await
            .is_err()
    );
    second
        .upload(
            file.path().to_path_buf(),
            "second.bin".to_owned(),
            FileLocation::Root,
            UploadOptions::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(first_transport.request_count(), 1);
    assert_eq!(second_transport.request_count(), 1);
    assert_eq!(
        second_transport.requests.lock().unwrap()[0].query[0].1,
        "second.bin"
    );
}

#[tokio::test]
async fn interrupted_upload_is_not_retried_and_the_client_remains_usable() {
    use crate::transport::{HttpRequest, HttpResponse, Transport};
    use async_trait::async_trait;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    #[derive(Debug, Default)]
    struct InterruptedTransport(AtomicUsize);

    #[async_trait]
    impl Transport for InterruptedTransport {
        async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse, SdkError> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(SdkError::Timeout)
            } else {
                Ok(response(200, "saved"))
            }
        }

        async fn download(
            &self,
            _request: HttpRequest,
            _destination: PathBuf,
        ) -> Result<(), SdkError> {
            unreachable!("upload-only firmware cannot download")
        }
    }

    let transport = Arc::new(InterruptedTransport::default());
    let client = DeviceClient::with_transport(DeviceKind::FolioBook, transport.clone());
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"data").unwrap();
    let upload = || {
        client.upload(
            file.path().to_path_buf(),
            "sample.bin".to_owned(),
            FileLocation::Root,
            UploadOptions::default(),
            None,
        )
    };
    assert_eq!(upload().await, Err(SdkError::Timeout));
    assert_eq!(transport.0.load(Ordering::SeqCst), 1);
    upload().await.unwrap();
    assert_eq!(transport.0.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn real_http_upload_preserves_filename_bytes_and_streams_the_original_body() {
    use std::{io::Read, net::TcpListener};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let payload = vec![0x5a; 128 * 1024];
    let expected = payload.clone();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut headers = Vec::new();
        let mut byte = [0];
        while !headers.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            headers.push(byte[0]);
        }
        let headers = String::from_utf8(headers).unwrap();
        assert!(
            headers
                .starts_with("POST /up?name=%E6%A0%B7%E4%BE%8B%20%2B%26%3F%23%25.pdf HTTP/1.1\r\n"),
            "{headers}"
        );
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("content-length: 131072\r\n")
        );
        assert!(!headers.to_ascii_lowercase().contains("multipart"));
        let mut body = vec![0; expected.len()];
        socket.read_exact(&mut body).unwrap();
        assert_eq!(body, expected);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });
    let client = DeviceClient::connect(
        DeviceKind::FolioBook,
        &format!("http://{address}"),
        Duration::from_secs(5),
    )
    .unwrap();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&payload).unwrap();
    let progress = Arc::new(RecordingProgress::default());
    let result = client
        .upload(
            file.path().to_path_buf(),
            "样例 +&?#%.pdf".to_owned(),
            FileLocation::Root,
            UploadOptions::default(),
            Some(progress.clone()),
        )
        .await;
    server.join().unwrap();
    assert_eq!(result.unwrap().entry.size, 131_072);
    assert_eq!(progress.0.lock().unwrap().last(), Some(&(131_072, 131_072)));
}
