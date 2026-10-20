use crate::test_support::MockTransport;
use crate::{DeviceClient, DeviceKind, SdkError};
use std::path::PathBuf;
#[tokio::test]
async fn unsupported_capability_does_not_send_a_request() {
    let transport = MockTransport::new(Vec::new());
    let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport.clone());

    let result = client
        .download("/book.epub".to_owned(), PathBuf::from("unused"))
        .await;

    assert!(matches!(result, Err(SdkError::Unsupported(_))));
    assert_eq!(transport.request_count(), 0);
}

#[tokio::test]
async fn settings_require_declared_capabilities() {
    let transport = MockTransport::new(Vec::new());
    let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport.clone());
    assert!(matches!(
        client.list_settings().await,
        Err(SdkError::Unsupported(_))
    ));
    assert_eq!(transport.request_count(), 0);
}

#[tokio::test]
async fn modifying_operations_share_one_serialization_lock() {
    use crate::transport::{HttpRequest, HttpResponse, Transport};
    use async_trait::async_trait;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug, Default)]
    struct ConcurrentTransport {
        active: AtomicUsize,
        maximum: AtomicUsize,
        calls: AtomicUsize,
    }
    #[async_trait]
    impl Transport for ConcurrentTransport {
        async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse, SdkError> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum.fetch_max(active, Ordering::SeqCst);
            tokio::task::yield_now().await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(crate::test_support::response(200, "{}"))
        }
        async fn download(
            &self,
            _request: HttpRequest,
            _destination: PathBuf,
        ) -> Result<(), SdkError> {
            Ok(())
        }
    }
    let transport = Arc::new(ConcurrentTransport::default());
    let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport.clone());
    let (first, second) = tokio::join!(
        client.delete("/a.epub".to_owned()),
        client.delete("/b.epub".to_owned())
    );
    first.unwrap();
    second.unwrap();
    assert_eq!(transport.maximum.load(Ordering::SeqCst), 1);
    assert_eq!(transport.calls.load(Ordering::SeqCst), 2);
}
