# InkReaderLink Android artifact

This Android library packages the UniFFI Kotlin bindings and the matching
`libcold04_inkreaderlink.so` binaries into a Maven Central AAR. UniFFI loads
the native library as `cold04_inkreaderlink`.

The native library and Kotlin bindings are generated together from the Rust
workspace by `../scripts/build-android-sdk.sh`. Do not update either one without
rebuilding the other.

Published coordinate:

```kotlin
implementation("com.cold04:inkreaderlink-uniffi:<version>")
```

The package requires Android API 28 or newer and includes `arm64-v8a`,
`armeabi-v7a`, and `x86_64` native libraries.

For Maven Central, publish a version tag from the SDK repository. For local App
development, see the root README's **SDK 引用指南** for the `local` and
`git.<SHA>` version conventions and the `mavenLocal()` publishing command.
