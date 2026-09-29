# PicoBook SDK

PicoBook SDK（Rust crate：`picobook_sdk`）是使用 Rust 编写的局域网阅读设备 SDK，通过 UniFFI 为
iOS 和 Android 提供调用接口。

SDK 负责设备发现、设备能力识别、文件传输和文件管理。上层应用主要负责
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
│   ├── picobook-core/
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
│   └── picobook-sdk/
│       └── src/lib.rs             # UniFFI 对外接口
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

## 开发状态

当前提供 Read Pico 与 CrossPoint 的文件和 Wi-Fi 操作，以及 CrossPoint 的字体、
OPDS 服务器和动态设置管理；具体能力见 [SDK 接口文档](docs/sdk/interface.md)。

`DeviceProfile.file_formats.font_upload_extensions` 返回字体管理接口接受的格式，
例如 CrossPoint 当前支持的 `cpfont`。上层 App 应读取该字段提供文件选择和过滤，
不要在 App 中固定字体格式。
