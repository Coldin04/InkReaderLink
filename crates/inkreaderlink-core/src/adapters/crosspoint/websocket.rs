//! `CrossPoint` WebSocket upload wire protocol.
use crate::{
    SdkError, UploadProgressSink,
    transport::{SharedTransport, WebSocketMessage},
};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{fs::File, io::AsyncReadExt, time::timeout};
use url::Url;

pub(super) async fn upload(
    transport: &SharedTransport,
    path: &Path,
    name: &str,
    destination: &str,
    progress: Option<Arc<dyn UploadProgressSink>>,
) -> Result<(), SdkError> {
    let mut url = Url::parse(&transport.origin().ok_or_else(|| {
        SdkError::Unsupported("transport does not expose its connection origin".to_owned())
    })?)
    .map_err(|e| SdkError::InvalidArgument(format!("invalid CrossPoint origin: {e}")))?;
    url.set_scheme(if url.scheme() == "https" { "wss" } else { "ws" })
        .map_err(|()| SdkError::InvalidArgument("invalid WebSocket scheme".to_owned()))?;
    url.set_port(Some(81))
        .map_err(|()| SdkError::InvalidArgument("invalid WebSocket port".to_owned()))?;
    let mut file = File::open(path).await.map_err(file_error)?;
    let size = file.metadata().await.map_err(file_error)?.len();
    let mut socket = transport.open_websocket(url.as_str()).await?;
    socket
        .send(WebSocketMessage::Text(
            format!("START:{name}:{size}:{destination}").into(),
        ))
        .await?;
    loop {
        let message = socket
            .receive()
            .await?
            .ok_or_else(|| SdkError::RemoteFailure("WebSocket closed before READY".to_owned()))?;
        if let WebSocketMessage::Text(text) = message {
            if text == "READY" {
                break;
            }
            if let Some(error) = text.strip_prefix("ERROR:") {
                return Err(remote_error(error));
            }
        }
    }
    let mut buffer = [0u8; 4096];
    let mut sent = 0u64;
    loop {
        let count = file.read(&mut buffer).await.map_err(file_error)?;
        if count == 0 {
            break;
        }
        socket
            .send(WebSocketMessage::Binary(buffer[..count].to_vec().into()))
            .await?;
        sent += count as u64;
        loop {
            match timeout(Duration::from_millis(1), socket.receive()).await {
                Err(_) => break,
                Ok(Err(error)) => return Err(error),
                Ok(Ok(None)) => {
                    return Err(SdkError::RemoteFailure(
                        "WebSocket closed before DONE".to_owned(),
                    ));
                }
                Ok(Ok(Some(message))) => {
                    if handle_message(message, progress.as_deref())? {
                        return if sent == size {
                            Ok(())
                        } else {
                            Err(SdkError::RemoteFailure(
                                "WebSocket completed before the entire file was sent".to_owned(),
                            ))
                        };
                    }
                }
            }
        }
    }
    loop {
        let message = socket
            .receive()
            .await?
            .ok_or_else(|| SdkError::RemoteFailure("WebSocket closed before DONE".to_owned()))?;
        if handle_message(message, progress.as_deref())? {
            return Ok(());
        }
    }
}
fn handle_message(
    message: WebSocketMessage,
    progress: Option<&dyn UploadProgressSink>,
) -> Result<bool, SdkError> {
    if let WebSocketMessage::Text(text) = message {
        if text == "DONE" {
            return Ok(true);
        }
        if let Some(error) = text.strip_prefix("ERROR:") {
            return Err(remote_error(error));
        }
        if let Some((sent, total)) = parse_progress(&text) {
            if let Some(progress) = progress {
                progress.report(sent, total);
            }
        }
    }
    Ok(false)
}
fn parse_progress(text: &str) -> Option<(u64, u64)> {
    let mut fields = text.strip_prefix("PROGRESS:")?.split(':');
    Some((fields.next()?.parse().ok()?, fields.next()?.parse().ok()?))
}
fn remote_error(message: &str) -> SdkError {
    if message.to_ascii_lowercase().contains("already") {
        SdkError::Conflict(message.to_owned())
    } else if message.to_ascii_lowercase().contains("disk full") {
        SdkError::InsufficientStorage
    } else {
        SdkError::RemoteFailure(message.to_owned())
    }
}
#[allow(clippy::needless_pass_by_value)]
fn file_error(error: std::io::Error) -> SdkError {
    SdkError::InvalidArgument(format!("file I/O failed: {error}"))
}
