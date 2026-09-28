pub mod crosspoint;
pub mod read_pico;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFilePage {
    pub entries: Vec<crate::FileEntry>,
    pub page: usize,
    pub total: usize,
    pub pages: usize,
}
