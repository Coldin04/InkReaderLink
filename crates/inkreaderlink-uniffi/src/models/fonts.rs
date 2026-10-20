use inkreaderlink_core::{FontCatalog, FontFamily, FontFile};

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkFontFile {
    pub name: String,
    pub size: u64,
}

impl From<FontFile> for SdkFontFile {
    fn from(file: FontFile) -> Self {
        Self {
            name: file.name,
            size: file.size,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkFontFamily {
    pub name: String,
    pub sizes: Vec<u32>,
    pub files: Vec<SdkFontFile>,
}

impl From<FontFamily> for SdkFontFamily {
    fn from(family: FontFamily) -> Self {
        Self {
            name: family.name,
            sizes: family.sizes,
            files: family.files.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkFontCatalog {
    pub max_families: u32,
    pub families: Vec<SdkFontFamily>,
}

impl From<FontCatalog> for SdkFontCatalog {
    fn from(catalog: FontCatalog) -> Self {
        Self {
            max_families: catalog.max_families,
            families: catalog.families.into_iter().map(Into::into).collect(),
        }
    }
}
