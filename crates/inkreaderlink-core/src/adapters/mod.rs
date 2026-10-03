pub mod crosspoint;
pub mod read_pico;
pub mod wegooo_cell_fork;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFilePage {
    pub entries: Vec<crate::FileEntry>,
    pub page: usize,
    pub total: usize,
    pub pages: usize,
}
