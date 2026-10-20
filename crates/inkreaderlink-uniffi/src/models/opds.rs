use inkreaderlink_core::{OpdsCredential, OpdsServer};

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkOpdsServer {
    pub index: u32,
    pub name: String,
    pub url: String,
    pub username: String,
    pub has_password: bool,
}

impl From<OpdsServer> for SdkOpdsServer {
    fn from(server: OpdsServer) -> Self {
        Self {
            index: server.index,
            name: server.name,
            url: server.url,
            username: server.username,
            has_password: server.has_password,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkOpdsCredential {
    pub index: Option<u32>,
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: Option<String>,
}

impl From<SdkOpdsCredential> for OpdsCredential {
    fn from(credential: SdkOpdsCredential) -> Self {
        Self {
            index: credential.index,
            name: credential.name,
            url: credential.url,
            username: credential.username,
            password: credential.password,
        }
    }
}
