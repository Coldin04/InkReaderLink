#[test]
fn client_and_transport_do_not_depend_on_firmware_protocols() {
    let client = include_str!("../src/client.rs");
    for forbidden in [
        "DeviceKind::",
        "Adapter::",
        "HttpRequest",
        "HttpResponse",
        "self.transport",
        "ACCESS_CODE",
        "/api/",
        "session_credentials:",
    ] {
        assert!(
            !client.contains(forbidden),
            "client must delegate protocol detail: {forbidden}"
        );
    }
    let transport = include_str!("../src/transport.rs");
    for forbidden in [
        "adapters::",
        "START:",
        "PROGRESS:",
        "X-Pico-Token",
        "X-Transfer-Token",
        "set_port(Some(81))",
    ] {
        assert!(
            !transport.contains(forbidden),
            "transport must only handle raw I/O: {forbidden}"
        );
    }
}
