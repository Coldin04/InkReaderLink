# 构建与产物体积

## Release 配置

工作区根目录的 `Cargo.toml` 为所有 `--release` 构建统一启用：

```toml
[profile.release]
opt-level = "s"
lto = "thin"
codegen-units = 1
```

`opt-level = "s"` 优先优化体积，Thin LTO 允许跨 crate 优化，单 codegen unit
减少代码生成单元之间的优化边界。Android 构建脚本与 iOS 调用方的 Cargo Release
构建自动使用此配置。LTO 与单 codegen unit 会增加构建耗时。

保留默认的 `panic = "unwind"`，使 UniFFI 和 SDK 的 Tokio 任务桥接继续捕获 panic
并返回错误。此配置不改变公开接口、支持的固件、ABI 或 HTTPS/WSS 支持。

## 绑定与符号裁剪

原生库与 Kotlin/Swift 绑定必须来自同一次源码构建。先从未裁剪符号的库生成绑定，
再裁剪用于分发的副本。不要在工作区 Release 配置中设置 `strip = "symbols"`：
UniFFI 的 `--library` 模式需要读取库中的元数据符号表。

Android 执行 `scripts/build-android-sdk.sh` 后，由 Android Gradle Plugin 在构建 AAR
时裁剪原生库符号。比较体积时，应使用同样裁剪过的 `.so` 或最终 AAR，不能将未裁剪
的 Cargo 输出与 AAR 内的库直接比较。AAR 中三个 ABI 的总量也不等于单 ABI APK
实际携带的 SDK 大小。

iOS 的 `.a` 与包含真机、模拟器 slice 的 XCFramework 是开发者分发产物；最终用户
下载体积应以 Release 导出的 IPA 及其 App Thinning 报告为准。调用方如需使用
`strip -S` 缩小静态库，应处理生成绑定后的分发副本，并保留原始库用于符号化和诊断，
随后验证真机与模拟器链接。

## 优化依据与验证

2026-10-20 基于 `cc8f722` 源码，使用 Rust 1.99.1、Android NDK 28.2.13676358
构建 `aarch64-linux-android`，对每组 `.so` 同样执行 `llvm-strip --strip-unneeded`：

| 配置 | 原生库字节数 | 相比默认 Release |
| --- | ---: | ---: |
| 默认 Release | 5,402,280 | — |
| `s` + Thin LTO + 单 codegen unit | 4,161,304 | 减少 23.0% |
| `z` + Thin LTO + 单 codegen unit | 3,988,696 | 减少 26.2% |

两组优化构建生成的 Kotlin/Swift 绑定及动态导出符号名称均与基线一致。
选择 `s` 保留循环向量化；`z` 虽然在本次测量中更小，但关闭循环向量化，尚未验证
上传和 TLS 吞吐，因此暂不采用。

以上结果仅代表该源码与工具链的 arm64 测量，其他 ABI、编译器版本、NDK 版本及 iOS
最终产物需要分别测量。构建成功和绑定一致不代表已验证运行性能；后续调整配置时，
还应比较上传、下载、WebSocket 进度及错误处理，并验证移动端 Release 构建。
