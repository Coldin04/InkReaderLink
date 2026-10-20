use crate::test_support::{MockTransport, response};
use crate::{DeviceClient, DeviceKind, FileLocation, SdkError, UploadOptions};
use std::io::Write;
use tempfile::NamedTempFile;

use crate::test_support::RecordingProgress;
use crate::transport::{
    HttpRequest, HttpResponse, Transport, WebSocketConnection, WebSocketMessage,
};
use async_trait::async_trait;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};
#[derive(Debug)]
struct SocketTransport {
    http: Arc<MockTransport>,
    frames: Mutex<VecDeque<WebSocketMessage>>,
    sent: Arc<Mutex<Vec<WebSocketMessage>>>,
    urls: Mutex<Vec<String>>,
}
struct Socket {
    frames: VecDeque<WebSocketMessage>,
    sent: Arc<Mutex<Vec<WebSocketMessage>>>,
    ready: bool,
}
#[async_trait]
impl WebSocketConnection for Socket {
    async fn send(&mut self, message: WebSocketMessage) -> Result<(), SdkError> {
        if matches!(message, WebSocketMessage::Binary(_)) && !self.ready {
            return Err(SdkError::RemoteFailure(
                "binary frame sent before READY".to_owned(),
            ));
        }
        self.sent.lock().unwrap().push(message);
        Ok(())
    }
    async fn receive(&mut self) -> Result<Option<WebSocketMessage>, SdkError> {
        let message = self.frames.pop_front();
        if matches!(&message, Some(WebSocketMessage::Text(text)) if text.as_str() == "READY") {
            self.ready = true;
        }
        Ok(message)
    }
}
#[async_trait]
impl Transport for SocketTransport {
    fn origin(&self) -> Option<String> {
        Some("http://reader.example.test".to_owned())
    }
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, SdkError> {
        self.http.execute(request).await
    }
    async fn download(&self, _request: HttpRequest, _destination: PathBuf) -> Result<(), SdkError> {
        Ok(())
    }
    async fn open_websocket(&self, url: &str) -> Result<Box<dyn WebSocketConnection>, SdkError> {
        self.urls.lock().unwrap().push(url.to_owned());
        Ok(Box::new(Socket {
            frames: std::mem::take(&mut *self.frames.lock().unwrap()),
            sent: self.sent.clone(),
            ready: false,
        }))
    }
}

#[tokio::test]
async fn websocket_upload_drives_raw_frames_and_preserves_device_failures() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"book").unwrap();
    for final_message in [Some("DONE"), Some("ERROR:disk full"), None] {
        let mut frames = VecDeque::from([
            WebSocketMessage::Text("READY".into()),
            WebSocketMessage::Text("PROGRESS:2:4".into()),
        ]);
        if let Some(message) = final_message {
            frames.push_back(WebSocketMessage::Text(message.into()));
        }
        let transport = Arc::new(SocketTransport {
            http: MockTransport::new(vec![response(200, "[]")]),
            frames: Mutex::new(frames),
            sent: Arc::default(),
            urls: Mutex::default(),
        });
        let client = DeviceClient::with_transport(DeviceKind::CrossPoint, transport.clone());
        let progress = Arc::new(RecordingProgress::default());
        let result = client
            .upload(
                file.path().to_path_buf(),
                "book.epub".to_owned(),
                FileLocation::Root,
                UploadOptions {
                    prefer_websocket: true,
                    ..UploadOptions::default()
                },
                Some(progress.clone()),
            )
            .await;
        match final_message {
            Some("DONE") => assert!(result.unwrap().used_websocket),
            Some(_) => assert_eq!(result, Err(SdkError::InsufficientStorage)),
            None => assert!(
                matches!(result, Err(SdkError::RemoteFailure(message)) if message.contains("before DONE"))
            ),
        }
        assert_eq!(
            *transport.urls.lock().unwrap(),
            ["ws://reader.example.test:81/"]
        );
        assert_eq!(
            *transport.sent.lock().unwrap(),
            [
                WebSocketMessage::Text("START:book.epub:4:/".into()),
                WebSocketMessage::Binary(b"book".to_vec().into())
            ]
        );
        assert_eq!(*progress.0.lock().unwrap(), [(2, 4)]);
    }
}
