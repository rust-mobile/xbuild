# cross

Cross is a cross platform build tool for cross platform rust and flutter projects.

## Scope and limitations of cross

### Android
Since cross platform frameworks implement their own resource management system there is only
limited support for android resources. Cross supports generating an icon mipmap and splash
screen. If this is insufficient for you project you likely need a different tool.

Most code is expected to be written in rust/dart. If you require flutter plugins written in
Kotlin/Java or integrate with existing Kotlin/Java code you likely need a different tool.

Currently not all manifest features are supported yet. If something is missing please open an
issue so we can prioritize.
