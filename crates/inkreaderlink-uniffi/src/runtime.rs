use crate::SdkOperationError;
use inkreaderlink_core::SdkError;
use std::{future::Future, sync::OnceLock};
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
    sdk_runtime()
        .spawn(future)
        .await
        .map_err(|error| SdkOperationError::RemoteFailure {
            detail: format!("SDK task failed: {error}"),
        })?
        .map_err(Into::into)
}
