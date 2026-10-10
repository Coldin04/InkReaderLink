pub mod crosspoint;
pub mod kiikoread;
pub mod read_pico;
pub mod whiteos;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFilePage {
    pub entries: Vec<crate::FileEntry>,
    pub page: usize,
    pub total: usize,
    pub pages: usize,
}
