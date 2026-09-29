# SDK Interface

## 设备能力

设备发现和识别完成后返回 `DeviceProfile`。上层应用应根据 capability 和
constraint 渲染功能，不应根据设备类型硬编码界面。

`picobook_core::config::DeviceDefinition` 是设备声明的统一配置入口。新增或调整
固件时，应在此处集中配置格式、约束和可选能力。`files.upload` 是所有受支持
设备的最低能力，由 SDK 保证存在；文件列表、删除、下载、目录、重命名、移动、
WebSocket 上传、Wi-Fi、字体和 OPDS 管理等能力均为可选配置。只支持上传的设备可直接使用
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
| `fonts.list` | 可列出已安装字体族 |
| `fonts.upload` | 可上传 `.cpfont` 字体文件 |
| `fonts.delete` | 可删除字体族 |
| `opds.list` | 可列出已保存 OPDS 服务器 |
| `opds.save` | 可新增或更新 OPDS 服务器 |
| `opds.delete` | 可按索引删除 OPDS 服务器 |
| `settings.list` | 可读取可编辑设置及控件元数据 |
| `settings.update` | 可提交部分设置变更 |

当前约束：

| 字段 | Read Pico | CrossPoint |
|---|---:|---:|
| `can_list_directories` | false | true |
| `can_choose_upload_directory` | false | true |

固件格式声明：

| 设备 | 普通文件上传接受 | 字体上传接受 | 设备原生可读 |
|---|---|---|---|
| Read Pico | EPUB、TXT | 无 | EPUB、TXT |
| CrossPoint | 任意文件 | `.cpfont` | EPUB、TXT、Markdown、XTC |

`accepts_any_upload_format` 表示上传接口是否接受任意扩展名；若为 false，使用
`upload_extensions` 过滤。`readable_extensions` 表示设备原生阅读格式。
这些字段属于设备定义，不分散写在 adapter 或上层 App 中。

`font_upload_extensions` 单独声明字体管理接口接受的文件扩展名（不含点号）。
CrossPoint 当前返回 `cpfont`，因为固件字体上传接口只接受 `.cpfont` 并校验
`CPFONT` 文件头；Read Pico 返回空列表。上层 App 应读取
`DeviceProfile.file_formats.font_upload_extensions` 提供文件选择提示和过滤，
不得硬编码字体扩展名。此字段与普通图书的 `upload_extensions` 分开，避免将
字体格式误认为设备可阅读的图书格式。

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
`saveWifiNetwork`、`deleteWifiNetwork`、`listFonts`、`uploadFont`、
`deleteFontFamily`、`listOpdsServers`、`saveOpdsServer`、`deleteOpdsServer`、
`listSettings` 和 `applySettings`。

实际建立连接时应调用 UniFFI 的 `connectAndVerify`。该入口按所选设备类型请求
设备信息接口，Read Pico 验证 `/info`，CrossPoint 验证 `/api/status`；只有 HTTP
请求成功且响应包含对应设备的信息字段时才返回 client。同步 `connect` 只创建
client，不代表设备已连接。

每个调用先检查 profile 中的 capability，未声明时返回 `Unsupported`。文件上传
和下载使用流式 I/O；修改操作在单个设备 client 内串行执行。

上传通过 `UploadOptions` 明确冲突策略。CrossPoint 的 `ReplaceWithBackup` 会先
将旧文件改名为 `<name>.back`，上传并按大小复核，成功后删除备份；失败时尝试
恢复，恢复失败返回 `RecoveryFailed`。

CrossPoint 字体接口使用 `/api/fonts`、`/api/fonts/upload` 和
`/api/fonts/delete`。`listFonts` 返回最大字体族数，以及每个字体族的名称、
字号和文件清单；`uploadFont(family, localPath, fileName)` 使用流式 multipart
上传一个 `.cpfont` 文件，`family` 作为表单字段发送。`deleteFontFamily` 删除
整个字体族。这些接口处理设备上的字体文件；普通文件上传格式声明不因此改变。

CrossPoint OPDS 接口使用 `/api/opds` 和 `/api/opds/delete`。
`listOpdsServers` 返回索引、名称、URL、用户名和 `hasPassword`，不返回密码。
固件省略可选的用户名或 `hasPassword` 时，SDK 分别返回空字符串和 `false`。
`saveOpdsServer` 的 `index` 为 `null` 时新增，提供索引时更新；更新时
`password` 为 `null` 会省略请求字段，让固件保留原密码。`deleteOpdsServer`
按索引删除。以上字体及 OPDS 能力仅在 CrossPoint profile 中声明，Read Pico
调用会返回 `Unsupported`。

## 动态设置

CrossPoint 声明 `settings.list` 和 `settings.update`。`listSettings()` 返回
`SettingsSnapshot`，其中每个 `SettingDescriptor` 含 `key`、`name`、`category`、
`kind` 和当前 `value`。`kind` 为 `Toggle`、带 `options` 的 `Choice`、带
`min/max/step` 的 `Number`，或 `Text`。`value` 使用对应的带类型变体；
`Choice` 的值是选项索引，`Number` 可为负数。设置项和选项由设备运行时提供，
字体和词典相关选项可能随着设备内容变化。

Android/iOS 应根据 `DeviceProfile.capabilities` 显示入口，再按 `kind` 动态渲染
开关、单选、整数输入或文本输入；`category` 用于分组，`name` 和 `options` 仅
作为普通文本显示。App 不应拼接 endpoint，也不应将标签或选项文字当成提交值。
编辑期间保留原始 `SettingsSnapshot`，保存时调用
`applySettings(expected, changes)`，仅提交变更项的 `key` 和带类型的目标值。
成功返回设备重新读取的新快照，页面应据此刷新控件。

SDK 在提交前重新读取设置并核对原始快照。若当前值、选项或约束已改变，返回
`Conflict`，App 应提示刷新后再编辑。SDK 拒绝未知/重复 key、类型不符、
枚举索引越界、数值越界或不符合步长、过长文本，并限制设置响应大小及描述符
数量。固件返回的设置元数据按不可信数据处理；UI 不应将其作为 HTML 执行。
若 POST 成功但重新读取失败，返回 `CommittedWithWarning`，App 应提示设置可能
已保存并允许刷新。Read Pico 未声明设置能力，调用返回 `Unsupported`。

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
