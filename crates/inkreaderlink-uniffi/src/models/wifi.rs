use inkreaderlink_core::{WifiCredential, WifiNetwork};

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkWifiNetwork {
    pub index: Option<u32>,
    pub ssid: String,
    pub has_password: bool,
    pub is_last_connected: bool,
}

impl From<WifiNetwork> for SdkWifiNetwork {
    fn from(network: WifiNetwork) -> Self {
        Self {
            index: network.index,
            ssid: network.ssid,
            has_password: network.has_password,
            is_last_connected: network.is_last_connected,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkWifiCredential {
    pub index: Option<u32>,
    pub ssid: String,
    pub password: Option<String>,
}

impl From<SdkWifiCredential> for WifiCredential {
    fn from(credential: SdkWifiCredential) -> Self {
        Self {
            index: credential.index,
            ssid: credential.ssid,
            password: credential.password,
        }
    }
}
