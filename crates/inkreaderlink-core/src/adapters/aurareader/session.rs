use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::adapters::{DeviceAdapter, FileUpload, FontUpload};
use crate::model::{ConflictPolicy, SettingKind, SettingValue, SettingsSnapshot};
use crate::transport::{HttpBody, HttpMethod, HttpRequest, SharedTransport};
use crate::validation::{join_location, validate_file_name};
use crate::{
    DeviceInfoField, FileEntry, FileKind, FileLocation, SdkError, SettingChange,
    SettingsValidationPolicy, UploadResult, WifiCredential, WifiNetwork,
};

use super::Info;

#[derive(Deserialize)]
struct BookPage {
    page: usize,
    total: usize,
    pages: usize,
    root: String,
    items: Vec<Book>,
}
#[derive(Deserialize)]
struct Book {
    name: String,
    size: u64,
}
#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    ok: Option<bool>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    progress_cleanup_failed: bool,
}
#[derive(Serialize)]
struct WifiBody<'a> {
    ssid: &'a str,
    password: &'a str,
}

pub(crate) struct Session {
    transport: SharedTransport,
}
impl Session {
    pub fn new(transport: SharedTransport) -> Self {
        Self { transport }
    }
}

impl Session {
    async fn info(&self, op: &str) -> Result<Info, SdkError> {
        let r = self
            .transport
            .execute(HttpRequest::new(HttpMethod::Get, "/info"))
            .await?;
        ensure(&r, op)?;
        Info::parse(&r.body)
    }
    async fn success(&self, request: HttpRequest, op: &str) -> Result<Reply, SdkError> {
        let r = self.transport.execute(request).await?;
        parse_reply(&r, op)
    }
    async fn list_all(&self) -> Result<Vec<FileEntry>, SdkError> {
        let mut page = 0;
        let mut entries = Vec::new();
        let mut declared_total = None;
        loop {
            let mut req = HttpRequest::new(HttpMethod::Get, "/books");
            req.query.push(("page".into(), page.to_string()));
            let r = self.transport.execute(req).await?;
            ensure(&r, "list books")?;
            let data: BookPage = serde_json::from_slice(&r.body).map_err(|e| {
                SdkError::RemoteFailure(format!("invalid AuraReader book list: {e}"))
            })?;
            if data.pages > 100_000 || data.page != page || (data.pages > 0 && page >= data.pages) {
                return Err(SdkError::RemoteFailure(
                    "invalid AuraReader book pagination".into(),
                ));
            }
            if declared_total
                .replace(data.total)
                .is_some_and(|total| total != data.total)
            {
                return Err(SdkError::RemoteFailure(
                    "AuraReader book count changed during pagination".into(),
                ));
            }
            for book in data.items {
                validate_file_name(&book.name).map_err(|_| {
                    SdkError::RemoteFailure("invalid file name in AuraReader book list".into())
                })?;
                entries.push(FileEntry {
                    path: format!("{}/{}", data.root.trim_end_matches('/'), book.name),
                    name: book.name,
                    size: book.size,
                    kind: FileKind::Book,
                });
            }
            page += 1;
            if page >= data.pages {
                break;
            }
        }
        if declared_total.is_some_and(|total| total != entries.len()) {
            return Err(SdkError::RemoteFailure(
                "AuraReader book pagination did not return the declared total".into(),
            ));
        }
        Ok(entries)
    }
    async fn upload_book(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        let FileUpload {
            local_path,
            file_name,
            location,
            options,
            size,
            progress,
        } = upload;
        if !matches!(location, FileLocation::Root) {
            return Err(SdkError::Unsupported(
                "AuraReader uploads to its configured library root".into(),
            ));
        }
        validate_file_name(&file_name)?;
        let policy = self.info("check upload policy").await?;
        if policy
            .file_limit
            .is_some_and(|limit| limit > 0 && size > limit)
        {
            return Err(SdkError::InvalidArgument(
                "file exceeds AuraReader upload limit".into(),
            ));
        }
        if size > policy.free_bytes {
            return Err(SdkError::InvalidArgument(
                "AuraReader reports insufficient free storage".into(),
            ));
        }
        let overwrite = match options.conflict_policy {
            ConflictPolicy::Fail => false,
            ConflictPolicy::OverwriteWhenSupported => true,
            ConflictPolicy::ReplaceWithBackup => {
                return Err(SdkError::Unsupported(
                    "AuraReader does not support backup-based replacement".into(),
                ));
            }
        };
        let mut check = HttpRequest::new(HttpMethod::Get, "/books");
        check.query.push(("name".into(), file_name.clone()));
        let existing = self.transport.execute(check).await?;
        if existing.status == 200 && !overwrite {
            return Err(SdkError::Conflict(format!(
                "AuraReader already has a file named {file_name}"
            )));
        }
        if existing.status != 200 && existing.status != 404 {
            return Err(remote("check existing book", &existing.body));
        }
        let mut req = HttpRequest::new(HttpMethod::Put, "/upload");
        req.query.push(("name".into(), file_name.clone()));
        if overwrite {
            req.query.push(("overwrite".into(), "1".into()));
        }
        req.body = HttpBody::RawFile {
            path: local_path,
            content_type: options.content_type,
        };
        let r = self
            .transport
            .execute_with_upload_progress(req, progress)
            .await?;
        parse_reply(&r, "upload book")?;
        Ok(UploadResult {
            entry: FileEntry {
                name: file_name.clone(),
                path: policy.storage_root.as_deref().map_or_else(
                    || join_location(&location, &file_name),
                    |root| format!("{}/{file_name}", root.trim_end_matches('/')),
                ),
                size,
                kind: FileKind::Book,
            },
            used_websocket: false,
        })
    }
    #[allow(clippy::too_many_lines)] // The fixed firmware settings schema is mapped in one place.
    fn setting_snapshot(info: &Info) -> Result<SettingsSnapshot, SdkError> {
        use SettingKind::{Choice as C, Number as N, Toggle as T};
        use SettingValue::{Choice as CV, Number as NV, Toggle as TV};
        let required = |v: Option<i64>, key: &str| {
            v.ok_or_else(|| SdkError::RemoteFailure(format!("AuraReader /info is missing {key}")))
        };
        let flag = |v: Option<bool>, key: &str| {
            v.map(TV).ok_or_else(|| {
                SdkError::RemoteFailure(format!("AuraReader /info is missing {key}"))
            })
        };
        let num = required(info.book_px, "book_px")?;
        let mut settings = vec![
            descriptor(
                "book_px",
                "正文字号",
                "文字",
                N {
                    min: 30,
                    max: 72,
                    step: 2,
                },
                NV(num),
            ),
            descriptor(
                "book_bold",
                "字重",
                "文字",
                T,
                flag(info.book_bold, "book_bold")?,
            ),
            descriptor(
                "reader_heading_right",
                "章节标题位置",
                "文字",
                T,
                flag(info.reader_heading_right, "reader_heading_right")?,
            ),
            descriptor(
                "reader_margin",
                "正文边距",
                "布局",
                C {
                    options: vec!["24".into(), "32".into(), "40".into(), "48".into()],
                },
                CV(choice(
                    required(info.reader_margin, "reader_margin")?,
                    &[0, 1, 2, 3],
                )?),
            ),
            descriptor(
                "reader_line_permille",
                "行距",
                "布局",
                C {
                    options: vec!["紧".into(), "标准".into(), "松".into()],
                },
                CV(choice(
                    required(info.reader_line_permille, "reader_line_permille")?,
                    &[1500, 1650, 1800],
                )?),
            ),
            descriptor(
                "reader_underline",
                "下划线",
                "布局",
                C {
                    options: vec!["无".into(), "虚线".into(), "实线".into()],
                },
                CV(choice(
                    required(info.reader_underline, "reader_underline")?,
                    &[0, 1, 2],
                )?),
            ),
            descriptor(
                "reader_indent",
                "首行缩进",
                "布局",
                T,
                flag(info.reader_indent, "reader_indent")?,
            ),
            descriptor(
                "reader_justify",
                "两端对齐",
                "布局",
                T,
                flag(info.reader_justify, "reader_justify")?,
            ),
            descriptor(
                "reader_chapter_opt",
                "章节优化",
                "布局",
                T,
                flag(info.reader_chapter_opt, "reader_chapter_opt")?,
            ),
            descriptor(
                "reader_blank_opt",
                "空行优化",
                "布局",
                T,
                flag(info.reader_blank_opt, "reader_blank_opt")?,
            ),
            descriptor(
                "reader_wave",
                "刷新波形模式",
                "刷新",
                C {
                    options: vec![
                        "清晰 (GC16)".into(),
                        "极速 (DU)".into(),
                        "平衡 (GL16)".into(),
                    ],
                },
                CV(choice(
                    required(info.reader_wave, "reader_wave")?,
                    &[0, 1, 2],
                )?),
            ),
            descriptor(
                "reader_full_every",
                "全刷频率",
                "刷新",
                C {
                    options: vec![
                        "每 5 页".into(),
                        "每 10 页".into(),
                        "每 20 页".into(),
                        "仅手动".into(),
                    ],
                },
                CV(choice(
                    required(info.reader_full_every, "reader_full_every")?,
                    &[5, 10, 20, 0],
                )?),
            ),
        ];
        settings.shrink_to_fit();
        Ok(SettingsSnapshot { settings })
    }
}

fn descriptor(
    key: &str,
    name: &str,
    category: &str,
    kind: SettingKind,
    value: SettingValue,
) -> crate::SettingDescriptor {
    crate::SettingDescriptor {
        key: key.into(),
        name: name.into(),
        category: category.into(),
        kind,
        value,
    }
}
fn choice(value: i64, allowed: &[i64]) -> Result<u32, SdkError> {
    allowed
        .iter()
        .position(|x| *x == value)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| SdkError::RemoteFailure("AuraReader setting index is too large".into()))?
        .ok_or_else(|| {
            SdkError::RemoteFailure("AuraReader reports an unsupported setting value".into())
        })
}
fn ensure(r: &crate::transport::HttpResponse, op: &str) -> Result<(), SdkError> {
    if (200..300).contains(&r.status) {
        Ok(())
    } else {
        Err(remote(op, &r.body))
    }
}
fn parse_reply(r: &crate::transport::HttpResponse, op: &str) -> Result<Reply, SdkError> {
    let reply: Reply = serde_json::from_slice(&r.body)
        .map_err(|e| SdkError::RemoteFailure(format!("invalid AuraReader {op} response: {e}")))?;
    if reply.progress_cleanup_failed {
        return Err(SdkError::CommittedButCleanupFailed(
            reply
                .error
                .unwrap_or_else(|| format!("AuraReader {op} committed but cleanup failed")),
        ));
    }
    if r.status != 200 || reply.ok == Some(false) {
        return Err(SdkError::RemoteFailure(reply.error.unwrap_or_else(|| {
            format!("AuraReader {op} failed with HTTP {}", r.status)
        })));
    }
    Ok(reply)
}
fn remote(op: &str, body: &[u8]) -> SdkError {
    let text = String::from_utf8_lossy(body);
    let message = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
        .unwrap_or_else(|| text.chars().take(256).collect());
    SdkError::RemoteFailure(format!("AuraReader {op} failed: {message}"))
}

#[async_trait]
impl DeviceAdapter for Session {
    async fn verify_connection(&self) -> Result<(), SdkError> {
        self.info("verify connection").await.map(|_| ())
    }
    async fn device_info(&self) -> Result<Vec<DeviceInfoField>, SdkError> {
        Ok(self.info("get device info").await?.fields())
    }
    async fn list_files(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        if !matches!(location, FileLocation::Root) {
            return Err(SdkError::Unsupported(
                "AuraReader does not support directory selection".into(),
            ));
        }
        self.list_all().await
    }
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        self.upload_book(upload).await
    }
    fn validate_delete(&self, path: &str) -> Result<(), SdkError> {
        let name = path.rsplit('/').next().unwrap_or(path);
        validate_file_name(name)
    }
    async fn delete(&self, path: &str) -> Result<(), SdkError> {
        let name = path.rsplit('/').next().unwrap_or(path);
        validate_file_name(name)?;
        let mut req = HttpRequest::new(HttpMethod::Delete, "/books");
        req.query.push(("name".into(), name.into()));
        self.success(req, "delete book").await.map(|_| ())
    }
    async fn list_wifi_networks(&self) -> Result<Vec<WifiNetwork>, SdkError> {
        let i = self.info("list Wi-Fi networks").await?;
        Ok(if i.wifi_configured {
            i.wifi_ssid
                .filter(|s| !s.is_empty())
                .map(|ssid| {
                    vec![WifiNetwork {
                        index: None,
                        ssid,
                        has_password: true,
                        is_last_connected: true,
                    }]
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        })
    }
    async fn save_wifi_network(&self, c: &WifiCredential) -> Result<(), SdkError> {
        if self.info("check Wi-Fi mode").await?.mode != "ap" {
            return Err(SdkError::Unsupported(
                "AuraReader Wi-Fi can only be changed in hotspot mode".into(),
            ));
        }
        let body = serde_json::to_vec(&WifiBody {
            ssid: &c.ssid,
            password: c.password.as_deref().unwrap_or(""),
        })
        .map_err(|e| SdkError::InvalidArgument(e.to_string()))?;
        let mut req = HttpRequest::new(HttpMethod::Post, "/wifi");
        req.body = HttpBody::Json(body);
        req.sensitive = true;
        self.success(req, "save Wi-Fi network").await.map(|_| ())
    }
    async fn delete_wifi_network(&self, _: Option<u32>) -> Result<(), SdkError> {
        if self.info("check Wi-Fi mode").await?.mode != "ap" {
            return Err(SdkError::Unsupported(
                "AuraReader Wi-Fi can only be changed in hotspot mode".into(),
            ));
        }
        self.success(
            HttpRequest::new(HttpMethod::Delete, "/wifi"),
            "forget Wi-Fi network",
        )
        .await
        .map(|_| ())
    }
    async fn upload_font(&self, u: FontUpload) -> Result<(), SdkError> {
        let FontUpload {
            local_path,
            file_name,
            overwrite,
            size,
            progress,
            ..
        } = u;
        validate_file_name(&file_name)?;
        let i = self.info("check font upload policy").await?;
        if !i.fonts_enabled {
            return Err(SdkError::Unsupported(
                "AuraReader font upload requires mounted TF storage".into(),
            ));
        }
        if i.font_limit.is_some_and(|n| n > 0 && size > n) {
            return Err(SdkError::InvalidArgument(
                "font exceeds AuraReader upload limit".into(),
            ));
        }
        if size > i.free_bytes {
            return Err(SdkError::InvalidArgument(
                "AuraReader reports insufficient free storage".into(),
            ));
        }
        let mut check = HttpRequest::new(HttpMethod::Get, "/fonts");
        check.query.push(("name".into(), file_name.clone()));
        let r = self.transport.execute(check).await?;
        if r.status == 200 && !overwrite {
            return Err(SdkError::Conflict(format!(
                "AuraReader already has a font named {file_name}"
            )));
        }
        if r.status != 200 && r.status != 404 {
            return Err(remote("check existing font", &r.body));
        }
        let mut req = HttpRequest::new(HttpMethod::Put, "/fonts");
        req.query.push(("name".into(), file_name));
        if overwrite {
            req.query.push(("overwrite".into(), "1".into()));
        }
        req.body = HttpBody::RawFile {
            path: local_path,
            content_type: None,
        };
        let r = self
            .transport
            .execute_with_upload_progress(req, progress)
            .await?;
        parse_reply(&r, "upload font")?;
        Ok(())
    }
    async fn list_settings(
        &self,
        _: SettingsValidationPolicy,
    ) -> Result<SettingsSnapshot, SdkError> {
        Self::setting_snapshot(&self.info("read settings").await?)
    }
    async fn apply_settings(
        &self,
        expected: SettingsSnapshot,
        changes: Vec<SettingChange>,
        _: SettingsValidationPolicy,
    ) -> Result<SettingsSnapshot, SdkError> {
        let current = self
            .list_settings(SettingsValidationPolicy::Permissive)
            .await?;
        if current != expected {
            return Err(SdkError::Conflict(
                "AuraReader settings changed since they were read".into(),
            ));
        }
        let mut body = serde_json::Map::new();
        body.insert("action".into(), "reading".into());
        for change in changes {
            let descriptor = current
                .settings
                .iter()
                .find(|s| s.key == change.key)
                .ok_or_else(|| {
                    SdkError::InvalidArgument(format!("unknown AuraReader setting {}", change.key))
                })?;
            let value = match (&descriptor.kind, change.value) {
                (SettingKind::Toggle, SettingValue::Toggle(v)) => serde_json::Value::Bool(v),
                (SettingKind::Number { min, max, step }, SettingValue::Number(v))
                    if v >= *min && v <= *max && *step > 0 && (v - min) % step == 0 =>
                {
                    serde_json::Value::Number(v.into())
                }
                (SettingKind::Choice { options }, SettingValue::Choice(n))
                    if usize::try_from(n).is_ok_and(|n| n < options.len()) =>
                {
                    let wire = match descriptor.key.as_str() {
                        "reader_margin" => [0, 1, 2, 3][n as usize],
                        "reader_line_permille" => [1500, 1650, 1800][n as usize],
                        "reader_underline" | "reader_wave" => [0, 1, 2][n as usize],
                        "reader_full_every" => [5, 10, 20, 0][n as usize],
                        _ => {
                            return Err(SdkError::InvalidArgument(
                                "unsupported AuraReader choice".into(),
                            ));
                        }
                    };
                    serde_json::Value::Number(wire.into())
                }
                _ => {
                    return Err(SdkError::InvalidArgument(format!(
                        "invalid value for AuraReader setting {}",
                        change.key
                    )));
                }
            };
            body.insert(change.key, value);
        }
        let mut req = HttpRequest::new(HttpMethod::Post, "/system");
        req.body = HttpBody::Json(
            serde_json::to_vec(&body).map_err(|e| SdkError::InvalidArgument(e.to_string()))?,
        );
        self.success(req, "save settings").await?;
        match self
            .list_settings(SettingsValidationPolicy::Permissive)
            .await
        {
            Ok(snapshot) => Ok(snapshot),
            Err(e) => Err(SdkError::CommittedWithWarning(format!(
                "settings were saved but could not be re-read: {e}"
            ))),
        }
    }
}
