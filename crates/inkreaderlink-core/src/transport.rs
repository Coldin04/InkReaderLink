//! Internal HTTP and WebSocket transport.

use std::{fmt, path::PathBuf, sync::Arc, time::Duration, time::Instant};

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use reqwest::{Body, Client, Method, multipart};
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    sync::Notify,
    time::{Instant as TokioInstant, sleep_until, timeout},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::io::ReaderStream;
use url::Url;

use crate::SdkError;
use crate::UploadProgressSink;

const UPLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
struct UploadActivity {
    last_byte_at: std::sync::Mutex<Instant>,
    changed: Notify,
}

impl UploadActivity {
    fn new() -> Self {
        Self {
            last_byte_at: std::sync::Mutex::new(Instant::now()),
            changed: Notify::new(),
        }
    }

    fn record(&self) {
        *self
            .last_byte_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Instant::now();
        self.changed.notify_one();
    }

    fn last_byte_at(&self) -> Instant {
        *self
            .last_byte_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

async fn send_with_upload_idle_timeout(
    builder: reqwest::RequestBuilder,
    activity: Arc<UploadActivity>,
) -> Result<reqwest::Response, SdkError> {
    let send = builder.send();
    tokio::pin!(send);
    loop {
        let deadline = TokioInstant::from_std(activity.last_byte_at() + UPLOAD_IDLE_TIMEOUT);
        let sleep = sleep_until(deadline);
        tokio::pin!(sleep);
        tokio::select! {
            result = &mut send => return result.map_err(map_reqwest_error),
            () = activity.changed.notified() => {},
            () = &mut sleep => {
                if activity.last_byte_at().elapsed() >= UPLOAD_IDLE_TIMEOUT {
                    return Err(SdkError::Timeout);
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpBody {
    Empty,
    Json(Vec<u8>),
    Form(Vec<(String, String)>),
    RawFile {
        path: PathBuf,
        content_type: Option<String>,
    },
    MultipartFile {
        path: PathBuf,
        file_name: String,
        field_name: String,
        content_type: Option<String>,
        fields: Vec<(String, String)>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: HttpBody,
    pub response_limit: Option<usize>,
}

impl HttpRequest {
    #[must_use]
    pub fn new(method: HttpMethod, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            body: HttpBody::Empty,
            response_limit: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    #[must_use]
    pub fn text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Clone)]
pub struct WebSocketUpload {
    pub file_path: PathBuf,
    pub file_name: String,
    pub destination: String,
    pub progress: Option<Arc<dyn UploadProgressSink>>,
}

impl fmt::Debug for WebSocketUpload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WebSocketUpload")
            .field("file_path", &self.file_path)
            .field("file_name", &self.file_name)
            .field("destination", &self.destination)
            .field("progress", &self.progress.is_some())
            .finish()
    }
}

#[async_trait]
pub trait Transport: Send + Sync + std::fmt::Debug {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, SdkError>;

    async fn execute_with_upload_progress(
        &self,
        request: HttpRequest,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<HttpResponse, SdkError> {
        let _ = progress;
        self.execute(request).await
    }

    async fn download(&self, request: HttpRequest, destination: PathBuf) -> Result<(), SdkError>;

    async fn upload_websocket(&self, upload: WebSocketUpload) -> Result<(), SdkError> {
        let _ = upload;
        Err(SdkError::Unsupported(
            "WebSocket upload is unavailable for this transport".to_owned(),
        ))
    }
}

#[derive(Clone, Debug)]
pub struct ReqwestTransport {
    base_url: Url,
    client: Client,
    upload_client: Client,
}

impl ReqwestTransport {
    /// Creates a transport for one device HTTP origin.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid URL or when the HTTP client cannot be built.
    pub fn new(base_url: &str, timeout: Duration) -> Result<Self, SdkError> {
        let normalized_url = if base_url.contains("://") {
            base_url.to_owned()
        } else {
            format!("http://{base_url}")
        };
        let mut base_url = Url::parse(&normalized_url)
            .map_err(|error| SdkError::InvalidArgument(format!("invalid device URL: {error}")))?;
        if base_url.scheme() != "http" && base_url.scheme() != "https" {
            return Err(SdkError::InvalidArgument(
                "device URL must use HTTP or HTTPS".to_owned(),
            ));
        }
        if base_url.host_str().is_none() {
            return Err(SdkError::InvalidArgument(
                "device URL must contain a host".to_owned(),
            ));
        }
        base_url.set_path("/");
        base_url.set_query(None);
        base_url.set_fragment(None);
        let client = Client::builder()
            .connect_timeout(timeout)
            .timeout(timeout)
            .build()
            .map_err(map_reqwest_error)?;
        // Match the firmware's browser uploader, which does not set an XHR
        // timeout. Reqwest 0.12 requires a finite request timeout, so use a
        // 24-hour ceiling while retaining the short connection timeout.
        let upload_client = Client::builder()
            .connect_timeout(timeout)
            .timeout(Duration::from_secs(24 * 60 * 60))
            .build()
            .map_err(map_reqwest_error)?;
        Ok(Self {
            base_url,
            client,
            upload_client,
        })
    }

    fn request_url(&self, request: &HttpRequest) -> Result<Url, SdkError> {
        let mut url = self
            .base_url
            .join(request.path.trim_start_matches('/'))
            .map_err(|error| SdkError::InvalidArgument(format!("invalid endpoint: {error}")))?;
        url.query_pairs_mut().extend_pairs(request.query.iter());
        Ok(url)
    }

    async fn request_builder(
        &self,
        request: HttpRequest,
        progress: Option<Arc<dyn UploadProgressSink>>,
        long_running_upload: bool,
        activity: Option<Arc<UploadActivity>>,
    ) -> Result<reqwest::RequestBuilder, SdkError> {
        let url = self.request_url(&request)?;
        let method = match request.method {
            HttpMethod::Get => Method::GET,
            HttpMethod::Post => Method::POST,
            HttpMethod::Put => Method::PUT,
            HttpMethod::Delete => Method::DELETE,
        };
        let client = if long_running_upload {
            &self.upload_client
        } else {
            &self.client
        };
        let builder = client.request(method, url);
        match request.body {
            HttpBody::Empty => Ok(builder),
            HttpBody::Json(body) => Ok(builder
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body)),
            HttpBody::Form(fields) => Ok(builder.form(&fields)),
            HttpBody::RawFile { path, content_type } => {
                let file = File::open(&path).await.map_err(map_file_error)?;
                let size = file.metadata().await.map_err(map_file_error)?.len();
                let stream = ReaderStream::new(file);
                let body = if let Some(progress) = progress {
                    let mut sent = 0_u64;
                    let activity = activity.clone();
                    Body::wrap_stream(stream.map(move |chunk| {
                        if let Ok(bytes) = &chunk {
                            sent = sent.saturating_add(bytes.len() as u64);
                            if let Some(activity) = &activity {
                                activity.record();
                            }
                            progress.report(sent.min(size), size);
                        }
                        chunk
                    }))
                } else if let Some(activity) = activity {
                    Body::wrap_stream(stream.map(move |chunk| {
                        if chunk.is_ok() {
                            activity.record();
                        }
                        chunk
                    }))
                } else {
                    Body::wrap_stream(stream)
                };
                let mut builder = builder
                    .header(reqwest::header::CONTENT_LENGTH, size)
                    .body(body);
                if let Some(content_type) = content_type {
                    builder = builder.header(reqwest::header::CONTENT_TYPE, content_type);
                }
                Ok(builder)
            }
            HttpBody::MultipartFile {
                path,
                file_name,
                field_name,
                content_type,
                fields,
            } => {
                let file = File::open(&path).await.map_err(map_file_error)?;
                let size = file.metadata().await.map_err(map_file_error)?.len();
                let stream = ReaderStream::new(file);
                let mut part = multipart::Part::stream_with_length(Body::wrap_stream(stream), size)
                    .file_name(file_name);
                if let Some(content_type) = content_type {
                    part = part.mime_str(&content_type).map_err(|error| {
                        SdkError::InvalidArgument(format!("invalid content type: {error}"))
                    })?;
                }
                let form = fields
                    .into_iter()
                    .fold(multipart::Form::new(), |form, (key, value)| {
                        form.text(key, value)
                    });
                Ok(builder.multipart(form.part(field_name, part)))
            }
        }
    }

    fn websocket_url(&self) -> Result<Url, SdkError> {
        let mut url = self.base_url.clone();
        url.set_scheme(if url.scheme() == "https" { "wss" } else { "ws" })
            .map_err(|()| SdkError::InvalidArgument("invalid WebSocket scheme".to_owned()))?;
        url.set_port(Some(81))
            .map_err(|()| SdkError::InvalidArgument("invalid WebSocket port".to_owned()))?;
        Ok(url)
    }
}

#[async_trait]
impl Transport for ReqwestTransport {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, SdkError> {
        let response_limit = request.response_limit;
        let response = self
            .request_builder(request, None, false, None)
            .await?
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = response.status().as_u16();
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(map_reqwest_error)?;
            if response_limit.is_some_and(|limit| chunk.len() > limit.saturating_sub(body.len())) {
                return Err(SdkError::RemoteFailure(
                    "response exceeds size limit".to_owned(),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse { status, body })
    }

    async fn execute_with_upload_progress(
        &self,
        request: HttpRequest,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<HttpResponse, SdkError> {
        let response_limit = request.response_limit;
        let activity = Arc::new(UploadActivity::new());
        let builder = self
            .request_builder(request, progress, true, Some(Arc::clone(&activity)))
            .await?;
        let response = send_with_upload_idle_timeout(builder, activity).await?;
        let status = response.status().as_u16();
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(map_reqwest_error)?;
            if response_limit.is_some_and(|limit| chunk.len() > limit.saturating_sub(body.len())) {
                return Err(SdkError::RemoteFailure(
                    "response exceeds size limit".to_owned(),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse { status, body })
    }

    async fn download(&self, request: HttpRequest, destination: PathBuf) -> Result<(), SdkError> {
        let response = self
            .request_builder(request, None, false, None)
            .await?
            .send()
            .await
            .map_err(map_reqwest_error)?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.map_err(map_reqwest_error)?;
            return Err(SdkError::RemoteFailure(format!("HTTP {status}: {body}")));
        }

        let temporary = destination.with_extension(format!(
            "{}.booksend-part",
            destination
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
        ));
        let mut output = File::create(&temporary).await.map_err(map_file_error)?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            output
                .write_all(&chunk.map_err(map_reqwest_error)?)
                .await
                .map_err(map_file_error)?;
        }
        output.flush().await.map_err(map_file_error)?;
        drop(output);
        tokio::fs::rename(&temporary, &destination)
            .await
            .map_err(map_file_error)
    }

    async fn upload_websocket(&self, upload: WebSocketUpload) -> Result<(), SdkError> {
        let size = tokio::fs::metadata(&upload.file_path)
            .await
            .map_err(map_file_error)?
            .len();
        let (socket, _) = connect_async(self.websocket_url()?.as_str())
            .await
            .map_err(|_| SdkError::Unreachable)?;
        let (mut sender, mut receiver) = socket.split();
        sender
            .send(Message::Text(
                format!("START:{}:{size}:{}", upload.file_name, upload.destination).into(),
            ))
            .await
            .map_err(map_websocket_error)?;
        expect_websocket_text(&mut receiver, "READY").await?;

        let mut file = File::open(&upload.file_path)
            .await
            .map_err(map_file_error)?;
        let mut buffer = vec![0_u8; 4096];
        loop {
            let read = tokio::io::AsyncReadExt::read(&mut file, &mut buffer)
                .await
                .map_err(map_file_error)?;
            if read == 0 {
                break;
            }
            sender
                .send(Message::Binary(buffer[..read].to_vec().into()))
                .await
                .map_err(map_websocket_error)?;
            drain_websocket_progress(&mut receiver, upload.progress.as_deref()).await?;
        }

        loop {
            let message = receiver
                .next()
                .await
                .ok_or_else(|| SdkError::RemoteFailure("WebSocket closed before DONE".to_owned()))?
                .map_err(map_websocket_error)?;
            if let Message::Text(text) = message {
                if text == "DONE" {
                    return Ok(());
                }
                if let Some((sent, total)) = parse_progress(&text) {
                    if let Some(progress) = upload.progress.as_deref() {
                        progress.report(sent, total);
                    }
                }
                if let Some(message) = text.strip_prefix("ERROR:") {
                    return Err(map_remote_message(message));
                }
            }
        }
    }
}

async fn drain_websocket_progress<S>(
    receiver: &mut S,
    progress: Option<&dyn UploadProgressSink>,
) -> Result<(), SdkError>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let next = timeout(Duration::from_millis(1), receiver.next()).await;
        let Ok(Some(message)) = next else {
            return Ok(());
        };
        let message = message.map_err(map_websocket_error)?;
        if let Message::Text(text) = message {
            if let Some((sent, total)) = parse_progress(&text) {
                if let Some(progress) = progress {
                    progress.report(sent, total);
                }
            } else if let Some(error) = text.strip_prefix("ERROR:") {
                return Err(map_remote_message(error));
            }
        }
    }
}

fn parse_progress(text: &str) -> Option<(u64, u64)> {
    let mut fields = text.strip_prefix("PROGRESS:")?.split(':');
    Some((fields.next()?.parse().ok()?, fields.next()?.parse().ok()?))
}

async fn expect_websocket_text<S>(socket: &mut S, expected: &str) -> Result<(), SdkError>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(message) = socket.next().await {
        if let Message::Text(text) = message.map_err(map_websocket_error)? {
            if text == expected {
                return Ok(());
            }
            if let Some(message) = text.strip_prefix("ERROR:") {
                return Err(map_remote_message(message));
            }
        }
    }
    Err(SdkError::RemoteFailure(format!(
        "WebSocket closed before {expected}"
    )))
}

fn map_remote_message(message: &str) -> SdkError {
    if message.to_ascii_lowercase().contains("already") {
        SdkError::Conflict(message.to_owned())
    } else if message.to_ascii_lowercase().contains("disk full") {
        SdkError::InsufficientStorage
    } else {
        SdkError::RemoteFailure(message.to_owned())
    }
}

#[allow(clippy::needless_pass_by_value)]
fn map_reqwest_error(error: reqwest::Error) -> SdkError {
    if error.is_timeout() {
        SdkError::Timeout
    } else if error.is_connect() {
        SdkError::Unreachable
    } else {
        // Reqwest's Display implementation usually only says "error sending
        // request for url". Include the source chain so stream and socket
        // failures remain diagnosable in SDK clients.
        SdkError::RemoteFailure(format!("{error:#}"))
    }
}

#[allow(clippy::needless_pass_by_value)]
fn map_websocket_error(error: tokio_tungstenite::tungstenite::Error) -> SdkError {
    SdkError::RemoteFailure(format!("WebSocket error: {error}"))
}

#[allow(clippy::needless_pass_by_value)]
fn map_file_error(error: std::io::Error) -> SdkError {
    SdkError::InvalidArgument(format!("file I/O failed: {error}"))
}

pub type SharedTransport = Arc<dyn Transport>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_qr_ip_hostname_and_full_url_inputs() {
        for input in ["192.168.4.1", "crosspoint.local", "http://192.168.4.1/"] {
            ReqwestTransport::new(input, Duration::from_secs(5)).unwrap();
        }
    }

    #[test]
    fn rejects_non_http_schemes() {
        let error = ReqwestTransport::new("ftp://192.168.4.1", Duration::from_secs(5));
        assert!(matches!(error, Err(SdkError::InvalidArgument(_))));
    }

    #[test]
    fn parses_crosspoint_progress_messages() {
        assert_eq!(
            parse_progress("PROGRESS:65536:123456"),
            Some((65_536, 123_456))
        );
        assert_eq!(parse_progress("DONE"), None);
        assert_eq!(parse_progress("PROGRESS:bad:123456"), None);
    }
}
