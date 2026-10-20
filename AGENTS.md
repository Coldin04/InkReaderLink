# InkReaderLink

## 项目

- 使用 Rust 编写，通过 UniFFI 提供 iOS 和 Android SDK。
- SDK 负责设备发现、能力识别、文件传输和文件管理；上层应用主要负责 UI 与平台原生入口。
- 支持横向增加其他设备和固件；新增设备且不增加通用能力时，不应要求上层修改调用代码。

## 架构

- 设备发现后返回设备信息和能力声明；上层按能力声明式展示功能。
- 各固件使用独立 adapter，SDK 内部按设备类型路由。
- SDK 对外 interface 和返回模型尽量设备无关、统一、通用。
- `inkreaderlink-core` 不依赖 UniFFI 或移动平台；`inkreaderlink-uniffi` 只负责将核心模型、错误和异步 API 映射到 UniFFI。
- UniFFI 按职责拆分模型映射、错误映射、运行时桥接、连接参数及各能力的导出方法；`lib.rs` 只保留模块声明、公开重导出和 scaffolding 入口。不得在单个入口文件堆积全部模型与方法，也不得在边界层复制固件协议。拆分后应比较 Swift/Kotlin 生成绑定，验证导出名称、参数、结果及错误接口兼容；绑定与原生库必须来自同一次源码构建。
- 所有固件的 endpoint、请求格式和错误语义只在各自 adapter 内处理。
- WebDAV、mDNS 等高阶功能暂不实现；设备专属高阶功能可使用显式设备类型的扩展函数。
- 文件传输必须流式处理；每台设备的修改操作串行执行。
- SDK 必须声明各设备允许上传和原生可读的文件格式，但不负责格式转换。
- SDK 必须通过 `DeviceProfile.file_formats.font_upload_extensions` 声明字体管理接口接受的格式；上层 App 读取此字段，不得硬编码字体扩展名。
- **固件实现必须集中在 `crates/inkreaderlink-core/src/adapters/<firmware>/` 目录。** 设备定义、格式与能力声明、连接地址解析、认证凭据、会话状态、endpoint、请求/响应格式、错误语义、分页、上传限制、握手、备份恢复和结果核对都属于对应固件 adapter。不得因历史代码已有特判而继续沿用或扩散。
- **`DeviceClient` 只负责通用能力门控、通用输入检查、修改操作串行化和统一接口委派。** 禁止在 `client.rs` 中引入具体固件的字段、凭据键、endpoint、adapter 请求构造器、协议解析、错误映射或按设备类型/品牌/身份字符串选择业务行为的分支。设备类型只在设备注册/adapter 工厂中用于选择实现。
- 复杂或多步操作必须通过与固件无关的类型化参数传给 adapter，由 adapter 执行完整流程并返回统一结果或错误；不得让 client 参与固件内部步骤，也不得把 client 本身传给 adapter 作为回调来绕过此规则。
- 适配器在创建连接时选定；修改 `DeviceProfile`、显示名称或能力声明不得更换通信协议。共用协议应由 adapter 之间明确组合，并显式传递存在差异的策略，不能在 client 或共享 transport 内按固件名称分支。
- `Transport` 只提供通用 HTTP、原始 WebSocket 消息、流式 I/O 和网络错误处理；禁止保存固件访问码/token，或实现固件专属路径、端口、握手消息、进度消息、响应正文和设备错误语义。
- 新增或修改 adapter 必须通过统一 client/adapter 接口验证能力门控、实际请求序列、会话隔离、失败与恢复语义；增加回归检查，防止固件分支或专属字段重新进入 client。对已有违反以上规则的实现，相关重构必须一并清理，不得仅改名、移动到共享文件或包装后保留原有耦合。
- 测试遵守相同职责划分：固件 endpoint、请求格式、握手、响应和错误语义的测试放在对应固件目录；client 仅测试通用门控、委派和并发控制；transport 仅测试通用编码、网络与流式 I/O。测试数据使用最小、明确的合成样例和保留测试域名/地址，不保留私人 IP、真实书名或用户文件路径；删除重复、失去覆盖价值或只复述实现的测试，保留能捕获真实回归的行为测试。
- 尽量保证SDK接口文档在写作后尽快在docs目录下更新和分类记录。
- 移动端产物命名：Android 原生库为 `libcold04_inkreaderlink.so`（加载名 `cold04_inkreaderlink`），iOS Swift 模块为 `InkReaderLink`；Rust crate 与 UniFFI Kotlin package 为 `inkreaderlink-uniffi` / `uniffi.inkreaderlink_uniffi`。重新生成绑定时必须与原生库一起更新，避免 ABI 不匹配。

## 固件

- `CrossPointAdapter`：CrossPoint HTTP、WebSocket 和文件管理。
- `IfInkOsAdapter`：if-ink OS 的目录上传、Markdown 导入、文件操作和 Wi-Fi 删除；共用 KiikoRead 的基础 HTTP 协议。
- `ReadPicoAdapter`：Read Pico HTTP、图书管理和部分成功语义。
- `PageRestAdapter`：普通设备地址连接，不使用访问码或 Pico Page token；按 PageRest 自己的 `/books` 与 `/info`、`/wifi` 协议处理分页、Wi-Fi、存储信息、上传预检查、原始文件上传、字体/图片目录与上传结果核对。网页标题中的“小纸 Pico”或“Read Pico”是界面文字，不是固件类型标识。
- `WhiteOsAdapter`：WhiteOS `/api` 文件列表、上传、下载和文件管理接口。
- `PicoPageAdapter`：Pico Page 路径访问码校验、会话 token 提取和文件上传；支持格式由设备定义声明。
- CrossPoint 文件替换可采用：旧文件改名为 `<name>.back`，上传新文件并核对成功后删除 `.back`；失败时尝试恢复，恢复失败必须返回明确错误。

## Git 与版本

- `master` 不直接开发；每次开发从分支或 fork 开始，通过合并或 PR 进入 `master`。
- 版本以 Git tag 管理。
- commit 必须签名。
- 未经用户明确同意，不得创建 commit；创建 commit 前必须先征得同意。
- 合并到 `master` 前，应对照 `.github/workflows/checks.yml` 检查改动涉及的静态检查要求，并修复已知会失败的项。Rust 改动重点检查 `cargo fmt --all -- --check` 和 Clippy `-D warnings`；无需等待 PR CI 的全部 job 完成或通过。若用户要求不运行编译或测试，可根据已有 CI 诊断与代码审查核对静态检查，但不得声称执行过未运行的检查。

## CI 发布

- 仅推送到 `master` 或目标分支为 `master` 的 Pull Request 运行提交检查；单独推送功能分支不运行。检查包含格式、Clippy、Rust 测试、脚本语法和 Android 绑定/AAR 构建，同一分支或 PR 的新提交会取消旧运行。fork PR 触发前应由 GitHub Actions 的首次贡献者审批设置把关；工作流只有 `contents: read` 权限。
- 提交检查不发布 Maven Central 或 GitHub Release；仅版本 tag 触发版本发布构建并发布 GitHub Release。
