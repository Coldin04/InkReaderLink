# InkReaderLink

InkReaderLink 是面向电子墨水阅读设备及兼容固件的设备桥接层（device bridge），为 App、未来的 Flutter 客户端和服务端提供统一设备能力。Rust 核心 crate `inkreaderlink-core` 管理设备能力、协议 adapter 和通用业务逻辑；UniFFI crate `inkreaderlink-uniffi` 将核心 API 映射为 iOS 与 Android 接口。

核心负责设备发现、设备能力识别、文件传输和文件管理。接入层负责
界面以及分享菜单等平台原生能力。不同固件通过内部 adapter 隔离，统一向
上层返回设备信息、能力声明、文件模型和操作结果。

## 当前目标

- 支持 Read Pico 和 CrossPoint。
- 支持横向增加其他设备和固件。
- 新增设备且不增加通用能力时，不要求上层修改调用代码。
- WebDAV、mDNS 等高阶能力暂不作为首版目标。

## 项目结构

```text
.
├── crates/
│   ├── inkreaderlink-core/
│   │   └── src/
│   │       ├── adapters/
│   │       │   ├── crosspoint.rs  # CrossPoint 协议 adapter
│   │       │   ├── read_pico.rs   # Read Pico 协议 adapter
│   │       │   └── mod.rs
│   │       ├── capability.rs      # 能力声明模型
│   │       ├── config.rs          # 设备能力、约束和格式配置
│   │       ├── client.rs          # 统一设备操作与 capability 门控
│   │       ├── device.rs          # 设备类型与路由
│   │       ├── model.rs           # 通用文件、上传和错误模型
│   │       ├── transport.rs       # 流式 HTTP/WebSocket transport
│   │       └── lib.rs
│   └── inkreaderlink-uniffi/
│       └── src/lib.rs             # 核心到 UniFFI API 的映射
├── docs/
│   ├── research/                  # 上游协议与技术选型调研
│   └── sdk/                       # SDK 对外接口约定
├── AGENTS.md                      # 项目开发约束
├── Cargo.toml
└── LICENSE
```

## 设计原则

- 上层根据能力声明决定是否展示可选功能。
- 上传是最低能力，其余能力均由设备定义按需配置。
- 文件格式声明集中在设备定义中；SDK 不负责格式转换。
- 固件 endpoint、请求格式和错误语义只在对应 adapter 内处理。
- 文件传输采用流式处理；设备修改操作串行执行。
- 对外接口和返回模型尽量设备无关、统一、通用。
- 核心 crate 不依赖 UniFFI 或移动平台；其他客户端和服务端可直接复用核心 crate 的异步 API、模型与错误。

## 开发状态

当前提供 Read Pico 与 CrossPoint 的文件和 Wi-Fi 操作，以及 CrossPoint 的字体、
OPDS 服务器和动态设置管理；具体能力见 [SDK 接口文档](docs/sdk/interface.md)。

`DeviceProfile.file_formats.font_upload_extensions` 返回字体管理接口接受的格式，
例如 CrossPoint 当前支持的 `cpfont`。上层 App 应读取该字段提供文件选择和过滤，
不要在 App 中固定字体格式。

## SDK 引用指南

Android App 使用 `com.cold04:inkreaderlink-uniffi`，依赖版本同时表达 SDK 的来源：

以下版本号均为格式示例，不代表当前版本或已发布版本；使用时应替换为实际的 SDK 版本和 commit SHA。

- 普通版本（例如 `0.2.0` 或 `0.2.0-preview.3`）从 Maven Central 获取。
- `git.<完整40位commit SHA>` 表示从 InkReaderLink Git 仓库的指定 commit 构建。CI 或本地辅助脚本会先构建 AAR 并发布到当前机器的 `mavenLocal()`，随后 App 按 Maven 坐标解析。
- `local` 表示使用开发者主动安装到 `mavenLocal()` 的本地 SDK。未安装时 App 构建会失败；编辑 SDK 源码不会自动覆盖已安装包。

### 安装当前工作区到 mavenLocal

在 SDK 仓库根目录运行：

```bash
./scripts/publish-android-sdk-local.sh local
```

只有执行这条命令后，App 才会看到当前工作区的改动。再次运行会用新的构建覆盖本机同名 `local` 版本。该版本只存在于当前机器，不会上传 Maven Central。

### 从指定 commit 安装到 mavenLocal

App 仓库提供 `scripts/install-sdk-from-git.sh`，按 SDK 坐标中的完整 SHA 临时检出源码、构建并安装到本地仓库。可在 App 仓库运行：

```bash
./scripts/install-sdk-from-git.sh git.0123456789abcdef0123456789abcdef01234567
```

命令中的 SHA 是格式示例，需要替换为实际存在的 40 位 commit SHA。

默认源码地址为 `https://github.com/Coldin04/InkReaderLink.git`。fork 或镜像仓库可通过 `INKREADERLINK_SDK_REPOSITORY` 覆盖。脚本使用临时目录，不会切换或改动你正在开发的 SDK 工作区。

GitHub App 构建流程使用同一来源约定：普通版本直接从 Maven Central 解析；`git.<SHA>` 版本按 SHA 构建 SDK 并缓存 AAR；`local` 版本会被拒绝，避免 CI 意外依赖某位开发者机器上的产物。更完整的 App 侧说明见 App 仓库 README 的“Android SDK 依赖与开发”章节。

## 提交检查

push 和 Pull Request 会运行 Rust 格式、Clippy、测试与脚本语法检查，并为 Android 的三个 ABI 生成 UniFFI 绑定和原生库、编译 AAR，核对包名和 AAR 内的库文件。该检查不会发布 Maven Central 或 GitHub Release；版本发布仍只由版本 tag 触发。
