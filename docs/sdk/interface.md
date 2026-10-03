# SDK Interface

## Crate 与平台边界

`inkreaderlink-core` 提供与平台无关的设备模型、错误、异步客户端和固件 adapters；它不依赖 UniFFI 或移动端运行时。`inkreaderlink-uniffi` 是移动端边界层，负责把核心模型、错误和异步操作映射为 UniFFI API。设备协议和能力策略只实现于核心 crate，其他入口应复用核心 API，而不是复制 adapter。

Android AAR 将同一 SDK 源码构建出的 Kotlin 绑定与 `libcold04_inkreaderlink.so` 一起打包。Kotlin 包为 `uniffi.inkreaderlink_uniffi`，JNI 加载名为 `cold04_inkreaderlink`；iOS Swift module 为 `InkReaderLink`。绑定或导出 ABI 改动必须同时重新生成绑定与原生库。

Flutter 接入可在现有核心之上增加 Dart FFI 或插件边界，服务端可直接依赖核心 crate 并选择合适的 async runtime。当前尚未为这两类入口增加代码或依赖。

## 设备能力

设备发现和识别完成后返回 `DeviceProfile`。上层应用应根据 capability 和
constraint 渲染功能，不应根据设备类型硬编码界面。

`inkreaderlink_core::config::DeviceDefinition` 是设备声明的统一配置入口。新增或调整
固件时，应在此处集中配置格式、约束和可选能力。`files.upload` 是所有受支持
设备的最低能力，由 SDK 保证存在；文件列表、删除、下载、目录、重命名、移动、
WebSocket 上传、Wi-Fi、字体和 OPDS 管理等能力均为可选配置。只支持上传的设备可直接使用
`DeviceDefinition::upload_only`。

### 支持设备与连接字段

移动端通过 `BooksendSdk.supportedDevices()` 获取 SDK 支持的固件列表。每个
`SdkSupportedDevice` 包含稳定的 `deviceType`、显示名称 `displayName` 和
`connectionFields`。设备选择列表和连接表单应使用这个目录，不应在 App 中维护另一份
固件名称或字段清单。

`SdkConnectionField` 以 `key` 标识保存值，包含显示标签 `label`、是否必填的
`required` 和输入类型 `kind`：`Text` 渲染文本框，`Address` 渲染地址输入框并允许
接入原生扫码，带 `options` 的 `Choice` 渲染下拉列表，`Toggle` 渲染开关。新增固件时，
在 `DeviceDefinition` 中声明显示名称和连接字段；只有 SDK 路由及 adapter 支持后，
才能把固件加入该目录。

连接表单值通过 `SdkConnectionParameter` 提交：文本和地址使用相应的 `SdkConnectionValue`
变体，下拉值使用从零开始的选项索引，开关使用布尔值。SDK 根据所选设备定义检查字段
是否存在、类型是否匹配、选项索引是否有效以及必填字段是否齐全。

当前 Read Pico 与 CrossPoint 都声明一个必填 `Address` 字段，key 为 `address`。
保存的连接值应随设备记录持久化；旧版仅保存 `address` 的记录可迁移为同名字段。
设备连接调用 `connectAndVerifyWithParameters(deviceType, parameters, timeoutMs)`；地址值由
SDK 声明的 `Address` 字段提供。原有 `connectAndVerify(deviceType, address, timeoutMs)`
仍可用于只需设备地址的调用。SDK 接受裸 IPv4/hostname 或完整 HTTP/HTTPS URL。

### 设备信息 capability

`device.info` 表示设备支持读取信息页字段。它是可选能力：设备定义可以不声明，
App 应只在 `DeviceProfile.capabilities` 包含此 ID 时显示信息页并调用 `deviceInfo()`。
未声明时调用返回 `Unsupported`。当前 Read Pico 和 CrossPoint 定义均声明此能力；
后续固件若不支持对应信息接口，应在设备定义中移除此 capability。

`deviceInfo()` 重新请求固件的信息接口，并返回 `SdkDeviceInfoField` 列表。每项包含
稳定字段 `key` 和字符串 `value`。Read Pico 字段 key 包括 `storage_is_flash`、
`storage_free_bytes`、`storage_file_limit`、`network_mode`、`wifi_configured`、
`wifi_ssid` 和 `storage_root`；CrossPoint 字段 key 包括 `firmware_version`、
`ip_address`、`network_mode`、`wifi_rssi`、`free_heap`、`uptime` 和 `device_model`。
固件未提供的可选字段会被省略。SDK 不提供本地化后的标题；App 应根据 `key` 映射
本地化标签，并按字段含义格式化值（例如字节数、运行时间及网络模式）。

`connectAndVerify` 仍会验证设备信息接口和设备身份字段。若某个固件完全没有该接口，
可用已知设备类型调用 `connect`；该入口不执行设备验证。此时应按固件对应的设备定义
省略 `device.info`，其他已声明能力仍可独立使用。

### 文件相关 capability

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
| `fonts.upload.progress` | SDK 可回传写入字体上传请求体的字节数 |
| `wifi.list` | 可读取固件暴露的已保存 Wi-Fi 信息 |
| `wifi.save` | 可保存或更新 Wi-Fi |
| `wifi.delete` | 可删除已保存 Wi-Fi |
| `fonts.list` | 可列出已安装字体族 |
| `fonts.upload` | 可上传设备声明的字体文件（Read Pico 为 `.ttf`，CrossPoint 为 `.cpfont`） |
| `fonts.upload.family` | 上传字体时必须提供字体族名称 |
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
| Read Pico | EPUB、TXT | `.ttf` | EPUB、TXT |
| CrossPoint | 任意文件 | `.cpfont` | EPUB、TXT、Markdown、XTC |

`accepts_any_upload_format` 表示上传接口是否接受任意扩展名；若为 false，使用
`upload_extensions` 过滤。`readable_extensions` 表示设备原生阅读格式。
这些字段属于设备定义，不分散写在 adapter 或上层 App 中。

`font_upload_extensions` 单独声明字体管理接口接受的文件扩展名（不含点号）。
CrossPoint 当前返回 `cpfont`，因为固件字体上传接口只接受 `.cpfont` 并校验
`CPFONT` 文件头；Read Pico 返回 `ttf`，固件只接受带 TrueType 轮廓的完整 TTF。上层 App 应读取
`DeviceProfile.file_formats.font_upload_extensions` 提供文件选择提示和过滤，
不得硬编码字体扩展名。此字段与普通图书的 `upload_extensions` 分开，避免将
字体格式误认为设备可阅读的图书格式。
仅当设备声明 `fonts.upload.family` 时，App 才要求用户输入字体族名称；Read Pico
直接按文件名上传，不需要此字段。

Read Pico 的 `/info` 只暴露单组已保存网络的 SSID，不提供多网络列表。
固件有 `/wifi` 保存和遗忘接口，但仅在设备热点模式允许调用；在已加入现有
网络的模式下会返回 403。SDK 保留 `wifi.list/save/delete` 能力以表达这些
实际存在的接口；App 应向用户解释热点模式限制，不应将其当作跨模式可用的
多网络管理。

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

`SdkDeviceClient` 统一提供：`deviceInfo`、`listFiles`、`upload`、`delete`、`download`、
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

### 多文件操作

`deleteFiles(paths)`、`moveFiles(paths, destination)` 和
`downloadFiles(files)` 接受多个文件路径。列表不能为空，且同一批次内不能重复路径；
下载还要求每个远端路径对应一个不同的本地目标路径。

SDK 按设备协议处理批次：CrossPoint 的删除使用 `/delete` 的 `paths` 参数一次发送；
Read Pico 的删除通过其单路径 `/books?name=...` 接口逐项串行执行。移动和下载当前都
通过单路径端点逐项串行执行，任一项失败即停止。错误详情包含已完成数量和失败路径；
先前已完成的文件操作不会回滚。Read Pico 未声明下载和移动能力，这两种调用仍返回
`Unsupported`。

上传通过 `UploadOptions` 明确冲突策略。CrossPoint 的 `ReplaceWithBackup` 会先
将旧文件改名为 `<name>.back`，上传并按大小复核，成功后删除备份；失败时尝试
恢复，恢复失败返回 `RecoveryFailed`。

Read Pico 仅声明 `fonts.upload`；`uploadFont(family, localPath, fileName)` 忽略
`family`，将 TTF 流式 PUT 至 `/fonts?name=...`，文件大小上限为 32 MiB，
且必须有已挂载 TF 卡。默认不替换同名字体；明确确认后可调用
`uploadFontWithOverwrite(family, localPath, fileName, true)`。固件会校验 TTF，
上传期间暂用内置字体，上传完成后仍需在设备上选用新字体。`GET /fonts`
只能按文件名查询单个字体，固件没有字体目录列表或删除接口，故不声明
`fonts.list` 和 `fonts.delete`。

Read Pico 还声明 `fonts.upload.progress`。带进度的字体上传回调反映 SDK 已写入
HTTP 请求体的文件字节数，达到文件大小后仍需等待固件校验并保存完成响应；
这不是设备端确认已落盘的进度。固件接收端维护的 `cur_bytes/cur_total` 当前
没有通过 HTTP 状态接口公开，SDK 不会将它们描述为设备端进度。

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
按索引删除。OPDS 能力仅在 CrossPoint profile 中声明，Read Pico 调用返回
`Unsupported`。

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
`Conflict`，App 应提示刷新后再编辑。设置 key 应唯一；作为对将同一设置放入多个
显示分类的固件兼容，若重复项的 key、名称、控件类型和当前值完全一致，SDK 保留首项
并忽略后项。其他重复 key 仍被拒绝。SDK 也拒绝未知 key、类型不符、
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
