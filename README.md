# cross

Cross is a cross platform build tool for rust and rust/flutter projects. It supports cross
compiling debug builds for all platforms.

## Scope and limitations of cross

- flutter plugins are not supported

Cross compiling release builds is currently not possible for a few reasons:

- dart/flutter lacks support for cross compiling aot snapshots
- creating an appimage relies on mksquashfs (linux only)
- creating a dmg relies on hdiutil (macos only)

### Android
Since cross platform frameworks implement their own resource management system there is only
limited support for android resources. Cross supports generating an icon mipmap and splash
screen. If this is insufficient for you project you likely need a different tool.

Most code is expected to be written in rust/dart. If you require flutter plugins written in
Kotlin/Java or integrate with existing Kotlin/Java code you likely need a different tool.

Currently not all manifest features are supported yet. If something is missing please open an
issue or PR.

### Linux
