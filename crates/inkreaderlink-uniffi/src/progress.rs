use inkreaderlink_core::UploadProgressSink;
use std::sync::Arc;
#[uniffi::export(foreign)]
pub trait SdkUploadProgressObserver: Send + Sync {
    fn on_progress(&self, sent_bytes: u64, total_bytes: u64);
}

pub(crate) struct FfiProgressSink {
    pub(crate) observer: Arc<dyn SdkUploadProgressObserver>,
}

impl UploadProgressSink for FfiProgressSink {
    fn report(&self, sent_bytes: u64, total_bytes: u64) {
        self.observer.on_progress(sent_bytes, total_bytes);
    }
}
