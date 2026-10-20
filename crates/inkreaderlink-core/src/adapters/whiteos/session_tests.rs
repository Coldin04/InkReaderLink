use crate::test_support::{MockTransport, response};
use crate::transport::HttpMethod;
use crate::{DeviceClient, DeviceKind, SdkError};
#[tokio::test]
async fn whiteos_delete_accepts_ok_response_and_sends_request_header() {
    let transport = MockTransport::new(vec![response(200, r#"{"ok":true}"#)]);
    let client = DeviceClient::with_transport(DeviceKind::WhiteOs, transport.clone());

    client.delete("/Books/book.epub".to_owned()).await.unwrap();

    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests[0].method, HttpMethod::Post);
    assert_eq!(requests[0].path, "/api/delete");
    assert_eq!(
        requests[0].query,
        [("path".to_owned(), "/Books/book.epub".to_owned())]
    );
    assert!(
        requests[0]
            .headers
            .iter()
            .any(|(name, value)| name == "X-Pico-Request" && value == "1")
    );
}

#[tokio::test]
async fn whiteos_delete_propagates_unsuccessful_json_response() {
    let transport = MockTransport::new(vec![response(
        200,
        r#"{"ok":false,"error":"delete failed"}"#,
    )]);
    let client = DeviceClient::with_transport(DeviceKind::WhiteOs, transport);

    let result = client.delete("/Books/book.epub".to_owned()).await;

    assert!(
        matches!(result, Err(SdkError::RemoteFailure(message)) if message.contains("delete failed"))
    );
}
