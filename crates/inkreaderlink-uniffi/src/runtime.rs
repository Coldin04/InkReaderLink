use crate::SdkOperationError;
use inkreaderlink_core::SdkError;
use std::{future::Future, sync::OnceLock};
use tokio::task::AbortHandle;

struct AbortOnDrop(AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
fn sdk_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("SDK Tokio runtime must be constructible")
    })
}

pub(crate) async fn run_on_sdk_runtime<T, F>(future: F) -> Result<T, SdkOperationError>
where
    T: Send + 'static,
    F: Future<Output = Result<T, SdkError>> + Send + 'static,
{
    let task = sdk_runtime().spawn(future);
    let abort_on_drop = AbortOnDrop(task.abort_handle());
    let result = task
        .await
        .map_err(|error| SdkOperationError::RemoteFailure {
            detail: format!("SDK task failed: {error}"),
        })?
        .map_err(Into::into);
    drop(abort_on_drop);
    result
}
