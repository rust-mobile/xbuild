# x
Cross is a build tool for rust and rust/flutter projects with support for cross compiling debug
builds and packaging/publishing for all major stores.

## Getting started
Create a new project:
```sh
x new helloworld

tree helloworld
helloworld
├── api.rsh
├── build.rs
├── Cargo.toml
├── lib
│   └── main.dart
├── pubspec.yaml
├── rust-toolchain.toml
└── src
    ├── lib.rs
    └── main.rs

2 directories, 8 files
```

List connected devices:
```sh
x devices
host                                              Linux               linux x64           Arch Linux 5.16.10-arch1-1
adb:16ee50bc                                      FP4                 android arm64       Android 11 (API 30)
imd:55abbd4b70af4353bdea2595bbddcac4a2b7891a      David’s iPhone      ios arm64           iPhone OS 15.3.1
```

Build or run on a device:
```sh
x build --device adb:16ee50bc
[1/9] Fetch flutter repo [SKIPPED]
[2/9] Fetch precompiled artefacts
[2/9] Fetch precompiled artefacts [7ms]
[3/9] Run pub get [SKIPPED]
[4/9] Build rust
    Finished dev [unoptimized + debuginfo] target(s) in 0.04s
[4/9] Build rust [112ms]
[5/9] Build classes.dex [SKIPPED]
[6/9] Build flutter assets [2ms]
[7/9] Build kernel_blob.bin [SKIPPED]
[8/9] Build aot snapshot [SKIPPED]
[9/9] Create apk [4316ms]
```

## Scope and limitations of x
Flutter plugins won't be supported.

Cross compiling release builds is currently not possible for a few reasons:

 - dart/flutter lacks support for cross compiling aot snapshots
 - creating an appimage relies on mksquashfs (linux only)
 - creating a dmg relies on hdiutil (macos only)

### Android
 - Building Android app bundles is not implemented yet (#6).
 - Improve debugging experience (#28).

### Ios
 - ios-deploy like tool (#40).

### Linux
 - Appimage signing has not been implemented yet (#5).
 - Linux sysroot to improve distro support and cross compilation has not been implemented yet (#14).

### Macos
 - Release build signing and notarization has not been implemented yet (#34).

### Windows
 - Msix packaging has not been implemented yet (#33).

## Troubleshooting

### Command not found
`x doctor` should let you know if there are any external tools missing.

```sh
x doctor
--------------------clang/llvm toolchain--------------------
clang                /usr/bin/clang
clang++              /usr/bin/clang++
llvm-ar              /usr/bin/llvm-ar
llvm-lib             /usr/bin/llvm-lib
lld                  /usr/bin/lld
lld-link             /usr/bin/lld-link
lldb                 /usr/bin/lldb
lldb-server          /usr/bin/lldb-server

----------------------------misc----------------------------
cargo                /usr/bin/cargo
git                  /usr/bin/git
flutter              /usr/bin/flutter

--------------------------android---------------------------
adb                  /opt/android-sdk/platform-tools/adb
javac                /usr/bin/javac
java                 /usr/bin/java

----------------------------ios-----------------------------
idevice_id           /usr/bin/idevice_id
ideviceinfo          /usr/bin/ideviceinfo
ideviceinstaller     /usr/bin/ideviceinstaller
ideviceimagemounter  /usr/bin/ideviceimagemounter
idevicedebug         /usr/bin/idevicedebug
```

### Dart Error: Can't load Kernel binary: Invalid SDK hash.
This happens when `flutter attach` and `x` use different flutter versions. To fix this run
`x update` in your project folder and `git checkout stable && git pull` in your flutter sdk.

### Generating apple signing key/certificate
See [apple_codesign_certificate_management](https://github.com/indygreg/PyOxidizer/blob/main/apple-codesign/docs/apple_codesign_certificate_management.rst) for further information.

### Generating mobile provisioning profiles
Without an apple developer account there is no cross platform way of generating mobile provisioning
profiles. You can either figure out how to generate it using xcode or [cook](https://github.com/n3d1117/cook).

## License
Apache-2.0 OR MIT
