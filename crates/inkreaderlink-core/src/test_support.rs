use crate::{
    SdkError, UploadProgressSink,
    transport::{HttpRequest, HttpResponse, Transport},
};
use async_trait::async_trait;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex},
};
#[derive(Debug)]
pub(crate) struct MockTransport {
    pub(crate) responses: StdMutex<VecDeque<HttpResponse>>,
    pub(crate) requests: StdMutex<Vec<HttpRequest>>,
    pub(crate) downloads: StdMutex<Vec<HttpRequest>>,
    pub(crate) upload_progress_observers: StdMutex<Vec<bool>>,
}

impl MockTransport {
    pub(crate) fn new(responses: Vec<HttpResponse>) -> Arc<Self> {
        Arc::new(Self {
            responses: StdMutex::new(responses.into()),
            requests: StdMutex::new(Vec::new()),
            downloads: StdMutex::new(Vec::new()),
            upload_progress_observers: StdMutex::new(Vec::new()),
        })
    }

    pub(crate) fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

#[async_trait]
impl Transport for MockTransport {
    fn origin(&self) -> Option<String> {
        Some("http://reader.example.test".to_owned())
    }

    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, SdkError> {
        self.requests.lock().unwrap().push(request);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| SdkError::RemoteFailure("missing mock response".to_owned()))
    }

    async fn execute_with_upload_progress(
        &self,
        request: HttpRequest,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<HttpResponse, SdkError> {
        self.upload_progress_observers
            .lock()
            .unwrap()
            .push(progress.is_some());
        if let Some(progress) = progress {
            progress.report(2, 4);
        }
        self.execute(request).await
    }

    async fn download(&self, request: HttpRequest, _destination: PathBuf) -> Result<(), SdkError> {
        self.downloads.lock().unwrap().push(request);
        Ok(())
    }
}

pub(crate) fn response(status: u16, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        body: body.as_bytes().to_vec(),
    }
}

#[derive(Default)]
pub(crate) struct RecordingProgress(pub(crate) StdMutex<Vec<(u64, u64)>>);

impl UploadProgressSink for RecordingProgress {
    fn report(&self, sent_bytes: u64, total_bytes: u64) {
        self.0.lock().unwrap().push((sent_bytes, total_bytes));
    }
}
