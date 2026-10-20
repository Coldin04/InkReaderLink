# SDK Interface

## Crate 与平台边界

`inkreaderlink-core` 提供与平台无关的设备模型、错误、异步客户端和固件 adapters；它不依赖 UniFFI 或移动端运行时。`inkreaderlink-uniffi` 是移动端边界层，负责把核心模型、错误和异步操作映射为 UniFFI API。设备协议和能力策略只实现于核心 crate，其他入口应复用核心 API，而不是复制 adapter。

`DeviceClient` 只执行通用输入校验、capability 门控、修改操作串行化和统一委派。
连接创建时由 adapter 工厂选定实现，随后替换 profile 仅改变设备声明，不改变协议。
连接地址和凭据分别通过 `configure_address` 与 `set_session_credentials` 交给该 adapter；
上传等操作以 `FileUpload`、`FontUpload`、`WallpaperUpload` 等内部类型化参数传递。
固件的地址解析、认证和会话、分页、预检查、限制、备份恢复与结果核对由 adapter 完整执行。

各固件代码位于 `adapters/<firmware>/`：`definition.rs` 声明设备能力和格式，
`mod.rs` 实现请求与响应编解码，`session.rs` 持有会话状态并编排协议流程；
CrossPoint 的原始 WebSocket 消息和动态设置编解码也位于其目录内。
固件协议测试与实现同目录；client/transport 测试仅覆盖通用行为。

UniFFI 的 `models/` 处理按能力分组的模型转换，`client/` 按能力导出统一操作，
`connection.rs` 处理声明式连接字段，`runtime.rs` 桥接 Tokio，`error.rs` 和
`progress.rs` 映射错误与进度。`lib.rs` 仅保留模块、重导出和 scaffolding。
移动 Rust 类型或导出方法到子模块可能改变 UniFFI 校验和，即使外部签名未变，也必须成套重建绑定和原生库。

Android AAR 将同一 SDK 源码构建出的 Kotlin 绑定与 `libcold04_inkreaderlink.so` 一起打包。Kotlin 包为 `uniffi.inkreaderlink_uniffi`，JNI 加载名为 `cold04_inkreaderlink`；iOS Swift module 为 `InkReaderLink`。绑定或导出 ABI 改动必须同时重新生成绑定与原生库。

Flutter 接入可在现有核心之上增加 Dart FFI 或插件边界，服务端可直接依赖核心 crate 并选择合适的 async runtime。当前尚未为这两类入口增加代码或依赖。

## 设备能力

设备发现和识别完成后返回 `DeviceProfile`。上层应用应根据 capability 和
constraint 渲染功能，不应根据设备类型硬编码界面。

`inkreaderlink_core::config::DeviceDefinition` 是设备声明的统一配置入口。新增或调整
固件时，应在对应 adapter 的 `definition.rs` 中配置格式、约束和可选能力，并由 `config` 注册。`files.upload` 是所有受支持
设备的最低能力，由 SDK 保证存在；文件列表、删除、下载、目录、重命名、移动、
WebSocket 上传、Wi-Fi、字体和 OPDS 管理等能力均为可选配置。只支持上传的设备可直接使用
`DeviceDefinition::upload_only`。

设备恒定信息也由 `DeviceDefinition` 维护，并随连接后的 `SdkDeviceProfile` 返回。
`displayResolution` 是可选的像素尺寸记录，包含 `width` 和 `height`；它表示 SDK 已知的
设备显示面板分辨率，不依赖设备在线信息接口。未知时为 `null`，App 不应自行推断。
当前 Read Pico 与 KiikoRead 声明为 684 × 1216（宽 × 高）；CrossPoint 暂无此字段。

### 支持设备与连接字段

移动端通过 `BooksendSdk.supportedDevices()` 获取 SDK 支持的固件列表。每个
`SdkSupportedDevice` 包含稳定的 `deviceType`、显示名称 `displayName` 和
`connectionFields`。设备选择列表和连接表单应使用这个目录，不应在 App 中维护另一份
固件名称或字段清单。

设备目录中的 CrossMux（`crossmux`）和 RickyOS（`rickyos`）是两个独立条目，分别保留设备类型
和显示名称；协议操作都路由到 CrossPoint adapter，并声明与 CrossPoint 相同的能力和格式。
if-ink OS（`if-ink-os`）使用独立 adapter 和 profile；`/info`、目录列表、字体、壁纸和 Wi-Fi 的
基础请求及响应解析复用 KiikoRead 协议，任意目标路径上传、Markdown 导入、文件变更和 Wi-Fi
删除使用 if-ink OS 专属请求。
PageRest（`pagerest`）是独立固件类型，不能因网页标题出现“小纸 Pico”或“Read Pico”而识别为 Read Pico。
它使用独立 adapter 和 PageRest 自己的图书分页、Wi-Fi 请求与响应解析；不使用 Pico Page 的访问码或 token。
连接只需普通设备地址，不使用 URL 访问码或 `X-Pico-Token`；也不复用 Pico Page 的会话处理。

FolioBook（`foliobook`，显示名 `FolioBook`）使用独立 adapter，仅声明 `files.upload`。
连接字段只有必填设备地址 `address`，保存责任为 `AppManaged`。上层通过现有设备目录和
统一上传 API 即可使用，无需增加固件专属调用。

`SdkConnectionField` 以 `key` 标识值，包含显示标签 `label`、是否在初次提交时必填的
`required`、输入类型 `kind` 和保存责任 `retention`：`Text` 渲染文本框，`Secret`
渲染密码框，`Address` 渲染地址输入框并允许接入原生扫码，带 `options` 的 `Choice`
渲染下拉列表，`Toggle` 渲染开关。`AppManaged` 值由 App 保存并在每次连接时提交；
`Session` 值只由 SDK 当前客户端保存，客户端销毁后失效。新增固件时，
在 `DeviceDefinition` 中声明显示名称和连接字段；只有 SDK 路由及 adapter 支持后，
才能把固件加入该目录。

连接表单值通过 `SdkConnectionParameter` 提交：文本、密码和地址使用相应的 `SdkConnectionValue`
变体，下拉值使用从零开始的选项索引，开关使用布尔值。SDK 根据所选设备定义检查字段
是否存在、类型是否匹配、选项索引是否有效以及必填字段是否齐全。

当前设备都声明一个必填 `Address` 字段，key 为 `address`。JustRead 的设备名称为
`JustRead`，另声明 `Secret` 字段 `access_code`（显示名“设备访问码”，
`Session`）。App 只保存 `AppManaged` 字段；旧版仅保存 `address` 的记录可迁移为同名字段。
设备连接调用 `connectAndVerifyWithParameters(deviceType, parameters, timeoutMs)`；地址值由
SDK 声明的 `Address` 字段提供。原有 `connectAndVerify(deviceType, address, timeoutMs)`
仍可用于只需设备地址的调用。SDK 接受裸 IPv4/hostname 或完整 HTTP/HTTPS URL。

Pico Page 同样声明可选 `Session` 凭据 `access_code`。地址可以是裸设备 origin，也可以是
设备二维码提供的完整 URL（例如 `http://192.168.0.120/j5za8a`）。完整 URL 的单段字母数字
路径由 adapter 解析为访问码；只输入 origin 时，连接探测返回 `CredentialRequired`，由 App
按声明弹出访问码输入框。若路径码错误，设备返回的“请扫描二维码或输入完整地址”页面会映射为
`CredentialRejected`，让 App 提示用户重新输入。访问码只在 SDK 当前会话中使用。

连接 JustRead 时先只提交地址。固件返回 401 后，SDK 返回可按错误变体识别的
`CredentialRequired { key: "access_code" }`；App 再按字段声明弹出密码输入，附上
`SdkConnectionValue.Secret` 重试连接。输入错误返回 `CredentialRejected`，两者都保留
字段 `key` 供 App 显示对应提示。连接成功后，SDK 当前客户端在后续请求中使用访问码；
App 不保存此码，下次连接重新探测。固定 API key 等长期值应声明为 `AppManaged`，
由 App 保存并在每次连接时传入。声明只包含显示名称和键名，不包含用户的键值。

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
| `wallpapers.upload.progress` | SDK 可回传写入图片上传请求体的字节数 |
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
| `wallpapers.upload` | 可上传图片到壁纸库，可选在同一请求中应用为锁屏壁纸 |
| `wallpapers.manage` | 可列出壁纸库内容并提供管理列表 |
| `wallpapers.delete` | 允许删除壁纸；仅影响管理列表中的删除操作入口 |

当前约束：

| 字段 | JustRead | Read Pico | CrossPoint | CrossMux / RickyOS | WhiteOS | KiikoRead 固件 | if-ink OS | PageRest |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `can_list_directories` | true | false | true | true | true | true | true | false |
| `can_choose_upload_directory` | true | false | true | true | false | true | true | false |

固件格式声明：

| 设备 | 普通文件上传接受 | 字体上传接受 | 壁纸上传接受 | 设备原生可读 |
|---|---|---|---|---|
| JustRead | 任意文件 | — | — | EPUB、TXT |
| Read Pico | EPUB、TXT | `.ttf` | — | EPUB、TXT |
| CrossPoint | 任意文件 | `.cpfont` | — | EPUB、TXT、Markdown、XTC |
| WhiteOS | API 接受任意扩展名 | — | — | TXT、EPUB、PDF |
| KiikoRead 固件 | EPUB、TXT | `.ttf`、`.otf` | `.jpg`、`.jpeg`、`.png` | EPUB、TXT |
| if-ink OS | EPUB、TXT、Markdown | `.ttf`、`.otf` | `.jpg`、`.jpeg`、`.png` | EPUB、TXT、Markdown |
| Pico Page | EPUB、TXT、TTF、OTF、PNG、JPEG | — | — | EPUB、TXT |
| PageRest | EPUB、TXT | `.ttf` | `.jpg`、`.jpeg`、`.png` | EPUB、TXT |
| FolioBook | 任意文件（包括 PDF） | — | — | 未确认，声明为空 |

FolioBook 按提供的上传网页协议工作：每个文件只发送一次
`POST /up?name=<URL 编码的文件名>`，请求体为原始文件流，支持统一上传进度回调。
仅 HTTP 200 表示成功，其他状态返回含 HTTP 状态及设备正文的 `RemoteFailure`。
传输失败直接返回网络错误，不自动重发，因为网页未提供上传结果核对接口。
进度表示本地文件已交给 HTTP 请求体的字节数，保存成功以最终 HTTP 响应为准。
网页未限制扩展名，因此允许 PDF 和其他文件上传；原生阅读格式尚未确认，
`readable_extensions` 留空表示未知。字体、壁纸管理、目录选择、覆盖和备份替换未声明；
默认冲突策略直接使用固件行为，SDK 无法预先检查同名文件，也不保证设备拒绝覆盖。

Pico Page 提供一个上传接口；SDK 仅按定义中声明的扩展名接受文件，并将文件流发送至设备。
设备负责决定文件保存位置和限制。SDK 在每个文件上传前重新读取已验证的传书页，从页面脚本提取
当前会话 token，并将同一 token 用于 `/check` 和 `/upload`；同时携带浏览器上传请求中的
`Origin` 与访问码页面 `Referer`。流式上传前通过 CRC-32 请求校验文件。非 200 响应保留设备
返回的 HTTP 状态和正文，不推断冲突、格式或存储错误。设备未声明文件列表、覆盖或目标目录能力。

PageRest 从普通设备地址读取 `/info`；图书列表使用自己的 `/books` 协议和分页响应模型。
上传前读取 `/info` 的可用状态、单文件限制和剩余空间，再向 `/upload-check?kind=...&name=...&size=...`
发送空 POST。若同名文件存在且调用方选择覆盖，SDK 在后续原始 `PUT` 中附加 `overwrite=1`；正文使用流式文件体，
不使用 multipart。图书、字体和图片分别上传到 `/upload`、`/fonts`、`/images`；错误响应的设备正文会保留。
字体和图片管理读取 `/fs?root=fonts|images` 并过滤目录项，删除分别调用 `/fonts?name=...` 和
`/images?name=...`。书籍、字体和图片都按原始文件流顺序上传，不使用 multipart，SDK 分别提供请求体字节进度回调。
PageRest 的字体接口按单文件管理，现有 `FontCatalog` 模型以文件名作为族名，
并在该设备上将 `max_families=0` 表示固件未报告族数量上限。上传连接中断时，SDK 对照预检查返回的序号和
`/info.last_upload` 的序号、文件名、总字节数及错误状态；只有全部匹配时才显示设备错误，否则返回“结果未确认”。
Wi-Fi 信息、保存和遗忘请求使用 PageRest 自己的 `/info` 与 `/wifi` 解析和序列化；保存/遗忘仅允许
`mode=ap`，不会自动切换或断开网络。
壁纸上传只存入图片库，`applyToLockScreen=true` 对 PageRest 返回 `Unsupported`。
PageRest 图片能力映射到 SDK 通用壁纸模型；HTTP 请求体进度代表本地文件已交给网络请求的字节数，设备确认保存仍以最终响应为准。
取消 UniFFI 异步调用会中止对应 SDK 任务和正在进行的请求；若取消发生在设备已提交而响应尚未返回时，结果需从对应内容列表确认。

`accepts_any_upload_format` 表示上传接口是否接受任意扩展名；若为 false，使用
`upload_extensions` 过滤。`readable_extensions` 表示设备原生阅读格式。
这些字段属于设备定义，不分散写在 adapter 或上层 App 中。

`font_upload_extensions` 单独声明字体管理接口接受的文件扩展名（不含点号）。
CrossPoint 当前返回 `cpfont`，因为固件字体上传接口只接受 `.cpfont` 并校验
`CPFONT` 文件头；Read Pico 返回 `ttf`，固件只接受带 TrueType 轮廓的完整 TTF。上层 App 应读取
`DeviceProfile.file_formats.font_upload_extensions` 提供文件选择提示和过滤，
不得硬编码字体扩展名。此字段与普通图书的 `upload_extensions` 分开，避免将
字体格式误认为设备可阅读的图书格式。

`wallpaper_upload_extensions` 单独声明壁纸上传接口接受的图片扩展名（不含点号）。
壁纸库操作由 `wallpapers.upload`、`wallpapers.manage` 和 `wallpapers.delete` 分别控制：
上传能力决定是否提供壁纸上传；管理能力决定是否提供壁纸列表；删除能力只决定列表中
是否显示删除按钮。SDK 的 `listWallpapers()` 返回 `/pictures` 下受支持格式的普通
图片文件；`uploadWallpaper(localPath, fileName, overwrite, applyToLockScreen)` 上传图片，
可选同时设为锁屏壁纸，并返回 `SdkWallpaperUploadResult`。单张上传上限为 20 MiB，
应用为锁屏壁纸时上限为 2 MiB。若固件已保存图片但设置锁屏壁纸失败，SDK 返回
`CommittedWithWarning`，表示文件已上传、应用步骤失败。`deleteWallpaper(fileName)`
按壁纸文件名删除。上述能力仅由 KiikoRead 固件声明。
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
WebSocket transport 只建立连接并收发原始消息；端口、握手、进度和设备错误消息由
相应 adapter 处理。请求是否包含敏感信息由 adapter 标记，transport 按通用标记隐藏调试输出。

文件位置和协议差异由对应 adapter 处理：

- `ReadPicoAdapter`：仅接受 `Root`。
- `PageRestAdapter`：图书仅接受 `Root`；字体和图片库通过各自的独立管理 API 列出，不暴露为通用文件目录。
- `FolioBookAdapter`：仅接受 `Root`，由固件决定存储卡中的保存位置。
- `CrossPointAdapter`：接受 `Root` 或 `Directory(path)`。
- `WhiteOsAdapter`：接受 `Root` 或 `Directory(path)`；列表接口一次返回完整目录。
- `KiikoReadAdapter`：接受 `Root` 或 `Directory(path)`；壁纸库列表固定读取 `pictures` 目录。
- `JustReadAdapter`：`Root` 列出 SD 卡根目录，默认上传到书架扫描的 `/books`；
  `Directory(path)` 可列出或上传到指定目录。

## 可调用 API

`SdkDeviceClient` 统一提供：`deviceInfo`、`listFiles`、`upload`、`delete`、`download`、
`createDirectory`、`rename`、`moveFile`、`listWifiNetworks`、
`saveWifiNetwork`、`deleteWifiNetwork`、`listFonts`、`uploadFont`、
`deleteFontFamily`、`listOpdsServers`、`saveOpdsServer`、`deleteOpdsServer`、
`listSettings`、`applySettings`、`listWallpapers`、`uploadWallpaper` 和
`deleteWallpaper`。

实际建立连接时应调用 UniFFI 的 `connectAndVerify`。该入口按所选设备类型验证设备：
Read Pico、KiikoRead 固件和 PageRest 检查 `/info`，CrossPoint 检查 `/api/status`，WhiteOS
读取根目录 `/api/list?path=/`，JustRead 读取根目录并处理访问码挑战。只有 HTTP 请求成功且响应符合对应协议格式时才返回 client。
同步 `connect` 只创建 client，不代表设备已连接；WhiteOS 没有设备信息接口。
FolioBook 通过 `GET /` 检查 `Folio Book` 网页标题与上传脚本标识，连接验证不会写入文件，
也不依赖设备信息接口或会话凭据。

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
`Conflict`，App 应提示刷新后再编辑。读取设置默认使用宽容策略：无法表示的单个设置项
会被跳过，重复 key 保留首次出现的设置项。需要诊断或严格兼容性检查的 core 调用方，
可在创建 `DeviceClient` 时显式选择 `SettingsValidationPolicy::Strict`；该模式会拒绝
无效设置项以及任何重复 key。无论策略如何，
损坏的 JSON、超出设置数量上限的响应仍会报错。提交时 SDK 始终拒绝未知 key、类型不符、
枚举索引越界、数值越界或不符合步长、过长文本。固件返回的设置元数据按不可信数据处理；
UI 不应将其作为 HTML 执行。
若 POST 成功但重新读取失败，返回 `CommittedWithWarning`，App 应提示设置可能
已保存并允许刷新。Read Pico 未声明设置能力，调用返回 `Unsupported`。

上传可传入 `SdkUploadProgressObserver`。CrossPoint WebSocket 上传时，固件每次
返回 `PROGRESS:<received>:<total>`，SDK 将设备已接收字节数回调给上层。
JustRead HTTP 上传时，SDK 回调已从本地文件流交给 HTTP 请求体的字节数，
与设备自带网页的上传进度同属客户端传输进度；到达文件大小仍须等待设备成功响应。
未收到回调的上传应显示不确定进度，`upload.websocket` 只表示可选用 WebSocket
传输，不表示是否存在上传进度回调。Read Pico 图书上传当前不承诺中途进度。

连接入口接受裸 IPv4/hostname（SDK 自动补 `http://`）或完整 HTTP/HTTPS URL。
二维码解析由原生 App 完成；Read Pico 网页码和 CrossPoint STA 网页码直接提供
IPv4 URL，CrossPoint AP 网页码可能提供 `crosspoint.local`。

## 文件列表

统一列表结果使用 `FileEntry`：`name`、规范化 `path`、`size` 和 `kind`。

- Read Pico 的分页由 SDK 内部处理，上层不感知固件分页参数。
- CrossPoint 的目录列表由 SDK 转换为相同的 `FileEntry` 列表。
- WhiteOS 的完整目录列表由 SDK 转换为相同的 `FileEntry` 列表。
- KiikoRead 固件的普通文件列表及壁纸列表分页由 SDK 内部处理。
- JustRead 最多返回 256 项且无分页；`truncated: true` 时 SDK 返回错误，不把不完整列表当作完整结果。
- App 不应拼接固件 endpoint；位置、分页和协议参数由 SDK 处理。

### WhiteOS 文件 API

WhiteOS 使用 `GET /api/list?path=...` 返回 `{ path, entries }`，每个条目包含
`name`、`dir` 和 `size`。该接口不分页；SDK 将 `dir` 映射为统一的目录类型，并根据
返回路径构造绝对文件路径。

文件操作使用 `PUT /api/upload?path=...`、`GET /api/download?path=...`、
`POST /api/delete?path=...`、`POST /api/rename?from=...&to=...` 和
`POST /api/mkdir?path=...`。写操作带 `X-Pico-Request: 1` 请求头；上传使用流式
请求体。成功响应中的 `{ "ok": true }` 被接受，`ok: false` 返回的错误会传递给调用方。
设备 API 会递归删除目录内容，这一行为由适配器封装，不单独暴露为 capability。

WhiteOS 声明目录列表、选择上传目录、删除、下载、重命名和创建目录能力；没有移动、
覆盖上传、Wi-Fi、字体或设备信息接口。上传 API 未限制扩展名，设备原生可读格式尚未
确认，因此 SDK 允许任意扩展名上传，并将可读格式留空。

### JustRead 文件传输

用户须先在设备的文件传输页面启动热点或局域网服务。该服务每次启动生成新的访问码；
未携码访问文件接口时返回 401。SDK 使用 `GET /api/list` 探测，在当前客户端会话中
通过 `X-Transfer-Token` 请求头发送访问码。访问码不写入设备定义或持久存储，
SDK 错误与调试输出也不包含其值。若服务重启使访问码失效，后续操作返回
`CredentialRejected { key: "access_code" }`，App 应断开并重新连接。

目录与文件路径按 UTF-8 编码后放入 Base64 请求头。上传使用流式
`POST /api/upload`，`Root` 目标为 `/books`，同名文件返回冲突且不覆盖。
下载先通过 `POST /api/download-ticket` 获取限时、一次性票据，再用该票据
流式请求 `GET /api/download-file`；SDK 只接受指向同一设备该端点的票据 URL。
固件没有删除、重命名、移动或创建目录接口，SDK 不声明对应能力。固件单文件上传
上限为 `0x7fff0000` 字节；SDK 可回调 HTTP 请求体的字节进度，回调达到 100%
不代表设备已完成保存，仍以 `/api/upload` 的成功响应为准。上传期间按实际读取
字节判断空闲：请求体传输停滞 60 秒、请求体完成后等待设备响应停滞 180 秒才超时；
以每秒一次的检查采样，无须对每次进度回调重设计时器。
