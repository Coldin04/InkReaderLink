pub mod aurareader;
pub mod crossmux;
pub mod crosspoint;
pub mod foliobook;
pub mod if_ink_os;
pub mod justread;
pub mod kiikoread;
pub mod pagerest;
pub mod pico_page;
pub mod read_pico;
pub mod rickyos;
pub mod whiteos;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFilePage {
    pub entries: Vec<crate::FileEntry>,
    pub page: usize,
    pub total: usize,
    pub pages: usize,
}

mod contract;
pub(crate) use contract::{DeviceAdapter, FileUpload, FontUpload, WallpaperUpload};

pub(crate) fn create(
    kind: crate::DeviceKind,
    transport: crate::transport::SharedTransport,
) -> Box<dyn DeviceAdapter> {
    use crate::DeviceKind;
    match kind {
        DeviceKind::ReadPico => Box::new(read_pico::session::Session::new(transport)),
        DeviceKind::CrossPoint => Box::new(crosspoint::session::Session::new(transport)),
        DeviceKind::WhiteOs => Box::new(whiteos::session::Session::new(transport)),
        DeviceKind::KiikoRead => Box::new(kiikoread::session::Session::new(transport)),
        DeviceKind::IfInkOs => Box::new(if_ink_os::session::Session::new(transport)),
        DeviceKind::JustRead => Box::new(justread::session::Session::new(transport)),
        DeviceKind::PicoPage => Box::new(pico_page::session::Session::new(transport)),
        DeviceKind::PageRest => Box::new(pagerest::session::Session::new(transport)),
        DeviceKind::FolioBook => Box::new(foliobook::session::Session::new(transport)),
        DeviceKind::AuraReader => Box::new(aurareader::session::Session::new(transport)),
    }
}
