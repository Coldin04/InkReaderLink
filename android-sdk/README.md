# PicoBookMgr Android artifact

This Android library packages the UniFFI Kotlin bindings and the matching
`libpicobookmgr.so` binaries into a Maven Central AAR.

The native library and Kotlin bindings are generated together from the Rust
workspace by `../scripts/build-android-sdk.sh`. Do not update either one without
rebuilding the other.

Published coordinate:

```kotlin
implementation("com.cold04:picobookmgr:<version>")
```

The package requires Android API 28 or newer and includes `arm64-v8a`,
`armeabi-v7a`, and `x86_64` native libraries.
