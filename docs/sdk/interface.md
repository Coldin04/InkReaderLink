# SDK Interface

## 设备能力

设备发现和识别完成后返回 `DeviceProfile`。上层应用应根据 capability 和
constraint 渲染功能，不应根据设备类型硬编码界面。

`picobook_core::config::DeviceDefinition` 是设备声明的统一配置入口。新增或调整
固件时，应在此处集中配置格式、约束和可选能力。`files.upload` 是所有受支持
设备的最低能力，由 SDK 保证存在；文件列表、删除、下载、目录、重命名、移动、
WebSocket 上传和 Wi-Fi 管理等能力均为可选配置。只支持上传的设备可直接使用
`DeviceDefinition::upload_only`。

当前文件相关 capability：

| ID | 含义 |
|---|---|
| `files.list` | 可列出文件 |
| `files.list.directories` | 文件列表包含目录 |
| `files.upload` | 可上传文件 |
| `upload.target-directory` | 上传时可选择目标目录 |
| `files.delete` | 可删除文件 |
| `files.download` | 可下载文件 |
| `files.rename` | 可重命名文件 |
| `files.move` | 可移动文件 |
| `directories.create` | 可创建目录 |
| `upload.explicit-overwrite` | 可显式覆盖上传 |
| `upload.backup-replace` | 可用 `.back` 备份替换 |
| `upload.websocket` | 可用 WebSocket 上传 |
| `wifi.list` | 可列出已保存 Wi-Fi |
| `wifi.save` | 可保存或更新 Wi-Fi |
| `wifi.delete` | 可删除已保存 Wi-Fi |

当前约束：

| 字段 | Read Pico | CrossPoint |
|---|---:|---:|
| `can_list_directories` | false | true |
| `can_choose_upload_directory` | false | true |

固件格式声明：

| 设备 | 上传接口接受 | 设备原生可读 |
|---|---|---|
| Read Pico | EPUB、TXT | EPUB、TXT |
| CrossPoint | 任意文件 | EPUB、TXT、Markdown、XTC |

`accepts_any_upload_format` 表示上传接口是否接受任意扩展名；若为 false，使用
`upload_extensions` 过滤。`readable_extensions` 表示设备原生阅读格式。
这些字段属于设备定义，不分散写在 adapter 或上层 App 中。

SDK 不提供格式转换。App 如需转换，应在调用 SDK 上传前使用其他依赖完成。

Read Pico 的 `Root` 表示固件当前选择的上传存储根。CrossPoint 的 `Root` 表示
文件系统根，也可使用 `Directory(path)` 指定列表或上传位置。

## Transport

`Transport` 仅为 SDK 内部 seam，负责 HTTP、WebSocket、流式请求体、超时和
取消。它不决定设备能力、文件位置或上传策略，也不通过 UniFFI 暴露。

文件位置和协议差异由对应 adapter 处理：

- `ReadPicoAdapter`：仅接受 `Root`。
- `CrossPointAdapter`：接受 `Root` 或 `Directory(path)`。

## 可调用 API

`SdkDeviceClient` 统一提供：`listFiles`、`upload`、`delete`、`download`、
`createDirectory`、`rename`、`moveFile`、`listWifiNetworks`、
`saveWifiNetwork` 和 `deleteWifiNetwork`。

每个调用先检查 profile 中的 capability，未声明时返回 `Unsupported`。文件上传
和下载使用流式 I/O；修改操作在单个设备 client 内串行执行。

上传通过 `UploadOptions` 明确冲突策略。CrossPoint 的 `ReplaceWithBackup` 会先
将旧文件改名为 `<name>.back`，上传并按大小复核，成功后删除备份；失败时尝试
恢复，恢复失败返回 `RecoveryFailed`。

CrossPoint WebSocket 上传可传入 `SdkUploadProgressObserver`。固件每次返回
`PROGRESS:<received>:<total>` 时，SDK 将真实的已接收字节数和总字节数回调给
上层；HTTP 与 Read Pico 上传当前不承诺中途进度。

连接入口接受裸 IPv4/hostname（SDK 自动补 `http://`）或完整 HTTP/HTTPS URL。
二维码解析由原生 App 完成；Read Pico 网页码和 CrossPoint STA 网页码直接提供
IPv4 URL，CrossPoint AP 网页码可能提供 `crosspoint.local`。

## 文件列表

统一列表结果使用 `FileEntry`：`name`、规范化 `path`、`size` 和 `kind`。

- Read Pico 的分页由 SDK 内部处理，上层不感知固件分页参数。
- CrossPoint 的目录列表由 SDK 转换为相同的 `FileEntry` 列表。
- App 不应拼接固件 endpoint；位置、分页和协议参数由 SDK 处理。
