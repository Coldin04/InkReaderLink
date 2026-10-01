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
- CrossPoint 与 Read Pico 的 endpoint、请求格式和错误语义只在各自 adapter 内处理。
- WebDAV、mDNS 等高阶功能暂不实现；设备专属高阶功能可使用显式设备类型的扩展函数。
- 文件传输必须流式处理；每台设备的修改操作串行执行。
- SDK 必须声明各设备允许上传和原生可读的文件格式，但不负责格式转换。
- SDK 必须通过 `DeviceProfile.file_formats.font_upload_extensions` 声明字体管理接口接受的格式；上层 App 读取此字段，不得硬编码字体扩展名。
- 尽量保证SDK接口文档在写作后尽快在docs目录下更新和分类记录。
- 移动端产物命名：Android 原生库为 `libcold04_inkreaderlink.so`（加载名 `cold04_inkreaderlink`），iOS Swift 模块为 `InkReaderLink`；Rust crate 与 UniFFI Kotlin package 为 `inkreaderlink-uniffi` / `uniffi.inkreaderlink_uniffi`。重新生成绑定时必须与原生库一起更新，避免 ABI 不匹配。

## 固件

- `CrossPointAdapter`：CrossPoint HTTP、WebSocket 和文件管理。
- `ReadPicoAdapter`：Read Pico HTTP、图书管理和部分成功语义。
- CrossPoint 文件替换可采用：旧文件改名为 `<name>.back`，上传新文件并核对成功后删除 `.back`；失败时尝试恢复，恢复失败必须返回明确错误。

## Git 与版本

- `master` 不直接开发；每次开发从分支或 fork 开始，通过合并或 PR 进入 `master`。
- 版本以 Git tag 管理。
- commit 必须签名。
- 未经用户明确同意，不得创建 commit；创建 commit 前必须先征得同意。

## CI 发布

- push 和 Pull Request 运行格式、Clippy、Rust 测试、脚本语法和 Android 绑定/AAR 检查；不发布 Maven Central 或 GitHub Release。
- 仅在创建版本 tag 时运行版本发布构建并发布 GitHub Release。
