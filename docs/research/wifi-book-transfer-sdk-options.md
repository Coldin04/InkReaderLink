# Read Pico 与 CrossPoint Wi-Fi 传书协议参考

调研日期：2026-09-28

> 请注意，此文由AI辅助编写，仅作为本SDK的技术参考，方便开发人员和AI查阅和制作SDK支持。

本文为移动端 SDK 技术选型提供上游事实依据。当前工作目录最初为空且不是 Git 仓库，没有既有文档约定，因此采用 `docs/research/`。所有源码引用均固定到调研时的 commit，避免后续分支变化导致结论漂移。

## 结论摘要

- 两套固件没有共同的 wire protocol，不应尝试用一组共享 DTO 直接兼容；应采用统一领域接口加两个独立 adapter。
- 共同的可靠基线是局域网明文 HTTP、串行上传、流式读文件和显式超时。CrossPoint 的 WebSocket 只能作为可选加速通道。
- Read Pico 的上传是原始字节 `PUT`，有结构化 JSON 错误、显式覆盖和临时文件原子提交；CrossPoint 是 multipart HTTP 或自定义 WebSocket，错误多为纯文本，当前源码不支持覆盖。
- 两者都没有 SDK 可依赖的用户鉴权或 TLS。App 必须把连接视为短期、局域网内、未认证会话，并在 UI 中清楚显示目标设备和网络。
- 发现能力不对称：CrossPoint 有 mDNS 与 UDP；Read Pico 依赖屏幕 URL/二维码。SDK 必须支持二维码/手输 IP 作为通用兜底。

## 上游快照

| 固件 | 调研 commit | 许可证 |
|---|---|---|
| Read Pico | [`88361427ccf3239ba2c7e4ec357cc981bbe9dee8`](https://github.com/MindReset/read_pico_firmware/tree/88361427ccf3239ba2c7e4ec357cc981bbe9dee8) | [Apache-2.0](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/LICENSE#L1-L15) |
| CrossPoint | [`93e98bb78702e29868a16a13b80c40e6b36ccdff`](https://github.com/crosspoint-reader/crosspoint-reader/tree/93e98bb78702e29868a16a13b80c40e6b36ccdff) | [MIT](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/LICENSE#L1-L21) |

## 能力对照

| 能力 | Read Pico | CrossPoint |
|---|---|---|
| 网络模式 | AP、STA | AP、STA、Calibre Wireless |
| AP | `ReadPico-<MAC 后四位>`，WPA2，固定密码 `readpico`，通常 `192.168.4.1` | `CrossPoint-Reader`，开放热点，通常 `192.168.4.1` |
| 发现 | 屏幕 URL/二维码；无 mDNS/UDP discovery | `crosspoint.local` mDNS；UDP 8134；屏幕 URL/二维码 |
| HTTP | 80，JSON 为主 | 80，JSON 与纯文本混合 |
| 主上传 | `PUT /upload`，raw body | `POST /upload`，multipart |
| 快速上传 | 无 | WebSocket 81，自定义文本控制协议加 binary frame |
| 文件管理 | 当前上传根下 TXT/EPUB 的分页查询、删除 | 任意目录列表、下载、建目录、重命名、移动、删除 |
| WebDAV | 无 | 有，Class 1 为主，LOCK/UNLOCK 仅兼容占位 |
| Wi-Fi 凭据 API | 仅 AP 模式下保存/遗忘单个网络 | 列出、增加、更新、删除多个已保存网络 |
| 鉴权/TLS | 无/无 | 无/无 |
| 覆盖 | 默认 409；`overwrite=1` 显式覆盖 | 文档称覆盖，当前 HTTP/WS 源码均拒绝同名文件 |
| 完整性确认 | 无 hash；原子临时文件提交 | 无 hash；WS 以声明长度完成，HTTP 直接写目标文件 |

## Read Pico

### 联网和发现

AP 与 STA 两种模式、固定 AP 密码及入口常量见[公共头文件](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/include/read_pico_transfer.h#L19-L25)。AP SSID 为 `ReadPico-%02X%02X`，使用 WPA2-PSK、channel 1、最多两个客户端；STA 使用设备保存的凭据，见[网络初始化](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L918-L931)。

固件没有 mDNS、DNS-SD 或 UDP discovery。设备通过屏幕显示 URL/二维码：AP 可在 Wi-Fi 加网码和网页 URL 码间切换，STA 显示 DHCP 地址 URL，见[二维码准备](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/main/apps/app_transfer.c#L59-L66)和[传书页展示逻辑](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/main/apps/app_transfer.c#L423-L455)。服务只在传书页运行，离开后会停止 HTTP/Wi-Fi 并释放资源，见[生命周期](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/main/apps/app_transfer.c#L489-L521)。

### HTTP 接口

路由均在端口 80 注册，完整表见[路由注册](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L947-L962)。

| Method | Path | 契约 |
|---|---|---|
| GET | `/` | 内嵌管理网页 |
| GET | `/info` | `{is_flash,free_bytes,file_limit,mode,wifi_configured,wifi_ssid,root}`；不返回密码，见[实现](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L597-L615) |
| PUT | `/upload?name=<encoded>[&overwrite=1]` | body 是文件原始字节，不是 multipart；成功通常为 `{"ok":true}` |
| GET | `/books?name=<encoded>` | 查询单本；存在时含 `{root,root_label,item:{name,size}}`，不存在为 404，见[实现](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L757-L780) |
| GET | `/books?page=<0-based>&q=<encoded>` | 每页固定 16 条，返回 `{root,root_label,page,total,pages,items}`；目录遍历未排序，见[分页实现](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L782-L804) |
| DELETE | `/books?name=<encoded>` | 删除文件，body 必须为空 |
| POST | `/books?name=<encoded>&action=retry_progress` | 文件已改变但阅读进度清理失败后的补偿操作，见[变更处理](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L806-L822) |
| POST | `/wifi` | 仅 AP；JSON `{ssid,password}`，保存后不自动切换 STA |
| DELETE | `/wifi` | 仅 AP；遗忘凭据，body 必须为空 |

Wi-Fi 配置对 SSID、密码和正文长度有严格限制，见[解析策略](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/transfer_policy.h#L51-L71)及[handler](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L660-L691)。

### 上传和错误语义

文件名最多 120 字节、须为规范 UTF-8，仅允许 `.txt`/`.epub`，并拒绝路径和 FAT 非法字符，见[文件名校验](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L31-L68)。请求必须有非零 `Content-Length`；413 表示单文件超限，507 同时覆盖空间不足和写入失败，见[长度检查](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L135-L139)。

上传先写 `<name>.part`，完成后 rename；覆盖时先保留 `.rename-backup`，提交失败会恢复旧文件，重启服务时还会清理孤立 part、恢复备份，见[清理与提交](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L157-L203)和[接收循环](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L209-L230)。默认不覆盖，存在时返回 409；只有 `overwrite=1` 才允许，见[冲突策略](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L248-L263)。内嵌客户端会先查询再确认，同时处理检查后的竞态 409，见[网页上传流程](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/upload.html#L52-L55)。

标准错误为 JSON：400、404、408、409、413、500、507，见[错误映射](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L572-L589)。需特别处理“部分成功”：文件已提交但阅读进度清理失败时为 HTTP 500，响应含 `committed:true` 或 `deleted:true` 与 `progress_cleanup_failed:true`；旧备份清理失败则为 200 加 warning。完整结构见[结果构造](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L729-L754)。SDK 不能把所有 500 都解释为“文件未保存”。

同一时间只允许一个 mutation；并发操作返回 409 或 503，见[准入逻辑](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L812-L838)。

### 安全边界

协议是明文 HTTP，无登录、token 或 TLS。`Host` 必须匹配设备公布的 IP；若有 `Origin` 也必须匹配，但原生客户端可不发 Origin。这是 DNS rebinding/跨源约束，不是用户鉴权，见[请求校验](https://github.com/MindReset/read_pico_firmware/blob/88361427ccf3239ba2c7e4ec357cc981bbe9dee8/components/read_pico_transfer/read_pico_transfer.c#L627-L648)。

## CrossPoint

### 联网和发现

服务仅在 File Transfer 或 Calibre Wireless 模式运行。Join Network 使用 2.4 GHz 网络；设备显示直接 IP、二维码和通常为 `crosspoint.local` 的 mDNS 地址。AP 是开放热点 `CrossPoint-Reader`，通常为 `192.168.4.1`，见[官方 Web Server Guide](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver.md#L8-L19)及[连接方式](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver.md#L32-L66)。源码确认 AP 无密码且会启动 captive-portal DNS，见[AP 配置](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/activities/network/CrossPointWebServerActivity.cpp#L22-L34)及[启动逻辑](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/activities/network/CrossPointWebServerActivity.cpp#L214-L264)。

UDP discovery 在 8134 接收精确文本 `hello`，回复 `crosspoint (on <hostname>);81`；末尾只给 WebSocket 端口，不含 IP，客户端从 UDP sender address 获取 IP，见[实现](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/network/CrossPointWebServer.cpp#L326-L346)。

### HTTP、WebSocket 与 WebDAV

官方 endpoint 文档列出 HTTP 80、WebSocket 81、UDP 8134 和 WebDAV 80，见[协议总览](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver-endpoints.md#L1-L12)。与传书直接相关的 HTTP 接口如下：

| Method | Path | 契约 |
|---|---|---|
| GET | `/api/status` | `{version,ip,mode,rssi,freeHeap,uptime,device}`，见[文档](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver-endpoints.md#L24-L54) |
| GET | `/api/files?path=/...` | 数组 `{name,size,isDirectory,isEpub}`，见[文档](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver-endpoints.md#L56-L82) |
| GET | `/download?path=...` | EPUB 为 `application/epub+zip`，其他为 octet-stream |
| POST | `/upload?path=...` | `multipart/form-data`，field 为 `file` |
| POST | `/mkdir`、`/rename`、`/move`、`/delete` | URL-encoded form；完整字段见[文档](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver-endpoints.md#L128-L196) |
| GET/POST | `/api/settings` | JSON 查询、JSON partial update |
| GET/POST | `/api/wifi`，POST `/api/wifi/delete` | 管理多条 Wi-Fi 凭据；GET 不返回密码，见[文档](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver-endpoints.md#L372-L417) |

源码注册的全部 route 见[路由表](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/network/CrossPointWebServer.cpp#L150-L205)。成功与失败响应并不统一：列表和状态是 JSON，大多数 mutation 是 `text/plain`；常见状态为 400/403/404/500。HTTP 上传成功为 200 纯文本，失败统一为 400 纯文本，见[上传结果](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/network/CrossPointWebServer.cpp#L810-L817)。

WebSocket 连接 `ws://<host>:81/`：发送 `START:<filename>:<size>:<path>`，等待 `READY` 后发送 binary frame；服务端每 64 KiB 或结束发送 `PROGRESS:<received>:<total>`，最终 `DONE` 或 `ERROR:<message>`，见[官方协议](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver-endpoints.md#L419-L462)及[源码状态机](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/network/CrossPointWebServer.cpp#L1608-L1717)。只有一个全局活动上传；溢出、写失败或断开会中止并删除不完整文件，见[binary 处理](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/network/CrossPointWebServer.cpp#L1722-L1779)。设备自带网页采用 4 KiB frame 和 `bufferedAmount` 背压，SDK 宜沿用该保守值，见[内嵌客户端](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/network/html/FilesPage.html#L5463-L5527)。只有连接建立失败时网页才 fallback 到 HTTP，见[fallback 逻辑](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/network/html/FilesPage.html#L5870-L5897)。

WebDAV 支持 `OPTIONS, GET, HEAD, PUT, DELETE, PROPFIND, MKCOL, MOVE, COPY, LOCK, UNLOCK`；PUT 用 `.davtmp` 后 rename，LOCK/UNLOCK 不提供完整 Class 2 锁，见[官方说明](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver-endpoints.md#L464-L480)。它适合文件管理器兼容，不适合首版 SDK 的核心上传通道。

### 文档与源码偏差

官方文档称同名 HTTP 上传会覆盖，见[文档](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver-endpoints.md#L102-L126)；但当前源码在 HTTP 上传时若目标存在就设置 `File already exists` 并返回 400，见[HTTP 实现](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/network/CrossPointWebServer.cpp#L692-L727)，WebSocket 同样返回 `ERROR:File already exists`，见[WS 实现](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/src/network/CrossPointWebServer.cpp#L1673-L1682)。SDK 应以版本化能力探测和真实响应为准，不应承诺覆盖。

另外，HTTP 上传直接创建目标文件，只有收到 `UPLOAD_FILE_ABORTED` 才删除；WebDAV 与 Read Pico 的临时文件策略更强。SDK 遇到 HTTP 超时后应重新列目录并核对 size，不能直接假定成功或失败。

### 安全边界

官方文档明确无鉴权，同网络任何人都能访问；HTTP 与 WS 均明文，AP 还是开放网络，服务只在相关页面存活，见[安全说明](https://github.com/crosspoint-reader/crosspoint-reader/blob/93e98bb78702e29868a16a13b80c40e6b36ccdff/docs/webserver.md#L139-L146)。Wi-Fi 密码不会由 GET API 返回，但 POST 仍通过明文局域网 HTTP 发送。

## SDK 技术选型输入

### 建议的模块边界

```text
BooksendClient
├── DeviceLocator          QR / 手输 IP / mDNS / CrossPoint UDP
├── DeviceProbe            识别厂商、版本、网络模式与能力
├── TransferSession        生命周期、可达性、超时、串行队列
├── DeviceAdapter
│   ├── ReadPicoAdapter    raw PUT + JSON/部分成功语义
│   └── CrossPointAdapter  multipart + WS 可选加速 + text/JSON 兼容
└── FileSource             流式文件、长度、文件名、进度、取消
```

不要让上层 App 直接看到 endpoint DTO。统一领域能力可定义为：`status`、`listBooks`、`checkExisting`、`upload(conflictPolicy)`、`delete`、`cancel`；目录、下载、重命名、Wi-Fi 凭据、WebDAV 等作为可选 capability。

### 推荐策略

1. 首版以两个原生 HTTP adapter 为基线；CrossPoint WebSocket 作为 feature flag/能力项，而不是唯一上传路径。
2. 每台设备串行执行 mutation。Read Pico 明确限制并发，CrossPoint WS 也只有一个全局上传状态。
3. 全程流式读取 URI/文件，不把 EPUB 整体读入内存。Read Pico 必须预先获得准确 `Content-Length`；CrossPoint WS 也要求声明总大小。
4. 冲突策略显式建模为 `fail`、`overwriteWhenSupported`、`replaceByDeleteThenUpload`。最后一种不是原子操作，必须由调用方明确同意；CrossPoint 当前版本不应静默 delete。
5. 统一错误模型至少区分：`transport`、`timeout`、`unreachable/serviceInactive`、`invalidRequest`、`conflict`、`insufficientStorage`、`unsupported`、`remoteFailure`、`committedWithWarning`、`committedButCleanupFailed`。
6. 上传成功后执行后验验证：重新查询同名项并核对大小。两套协议都没有 checksum；CrossPoint HTTP 超时尤其需要 reconciliation。
7. 发现顺序建议为：扫描设备二维码/URL > 已知 IP 快速探测 > CrossPoint mDNS > CrossPoint UDP > 手输 IP。不要用 SSID 推断设备身份。
8. 能力探测应缓存到 session，不永久绑定 firmware：Read Pico 用 `/info`；CrossPoint 用 `/api/status`，再按需试探 WS/endpoint。响应解析需宽松读取、严格输出。
9. 不在 SDK 日志记录 Wi-Fi 密码、完整 URL query 或文件正文。设备无 TLS/鉴权，默认只允许局域网/热点地址，并把目标 IP 与设备类型暴露给 UI 确认。

### 共享实现选择的约束

- 若采用 Kotlin Multiplatform，共享层适合承载状态机、adapter、序列化、错误归一化和测试；iOS/Android 各自负责本地网络权限、mDNS/UDP、文件 URI 与后台任务桥接。
- 若分别用 Swift/Kotlin 实现，平台网络与文件 API 最直接，但协议修复和测试用例要维护两份。
- 不建议首版使用 Rust/UniFFI：这里的难点主要是平台网络权限、文件句柄和行为不一致，而不是 CPU 密集逻辑；FFI 会增加流式 I/O、取消和 async 桥接成本。
- 无论选择哪种共享方案，都应建立 firmware contract fixtures：两套成功/错误响应、Read Pico 部分成功、CrossPoint 文本错误、断线、超时、同名冲突和旧版本缺字段。

## 仍需真机验证

- iOS/Android 在 AP 无互联网、STA client isolation、mDNS 不可用时的发现成功率。
- 大 EPUB 在弱信号下的超时阈值、后台切换、取消和设备退出传书页行为。
- CrossPoint 不同已发布 firmware 的覆盖行为与 WS 支持范围；当前 `develop` 文档和源码已经不一致。
- Read Pico 内置存储与 TF 卡的真实 `file_limit`、可用空间变化及非 ASCII 文件名互操作。
- 两套固件在成功响应丢失后的目录/size reconciliation，尤其是“服务已提交但客户端超时”。
