# Fast Booksend SDK

Fast Booksend SDK 是使用 Rust 编写的局域网阅读设备 SDK，通过 UniFFI 为
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
│   ├── booksend-core/
│   │   └── src/
│   │       ├── adapters/
│   │       │   ├── crosspoint.rs  # CrossPoint 协议 adapter
│   │       │   ├── read_pico.rs   # Read Pico 协议 adapter
│   │       │   └── mod.rs
│   │       ├── capability.rs      # 能力声明模型
│   │       ├── device.rs          # 设备类型与路由
│   │       └── lib.rs
│   └── booksend-ffi/
│       └── src/lib.rs             # UniFFI 对外接口
├── docs/research/                 # 上游协议与技术选型调研
├── AGENTS.md                      # 项目开发约束
├── Cargo.toml
└── LICENSE
```

## 设计原则

- 上层根据能力声明决定是否展示可选功能。
- 固件 endpoint、请求格式和错误语义只在对应 adapter 内处理。
- 文件传输采用流式处理；设备修改操作串行执行。
- 对外接口和返回模型尽量设备无关、统一、通用。

## 开发状态

当前为 Rust/UniFFI MVP 骨架阶段，尚未提供完整设备通信实现。
