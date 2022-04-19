# xbuild
xbuild is a build tool for rust and rust/flutter projects with support for cross compiling debug
builds and packaging/publishing for all major stores. The goal of xbuild is making native app development
as easy as web development.

## Getting started
Install `xbuild`:
```sh
cargo install xbuild
```

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

![x](https://user-images.githubusercontent.com/741807/162616805-30b48faa-84f0-4fec-851a-4c94fd35c6bd.png)

## Scope and limitations of x
Flutter plugins won't be supported.

Cross compiling release builds is currently not possible for a few reasons:

 - dart/flutter lacks support for cross compiling aot snapshots
 - creating a dmg relies on hdiutil (macos only)

### Android
 - Building Android app bundles is not implemented yet (#6).
 - Improve debugging experience (#28).

### Ios
 - ios-deploy like tool (#40).

### Linux
 - Appimage signing has not been implemented yet (#5).

### Windows
 - Msix packaging has not been implemented yet (#33).

## Troubleshooting

### Command not found
`x doctor` should let you know if there are any external tools missing.

```sh
x doctor
--------------------clang/llvm toolchain--------------------
clang                13.0.1              /usr/bin/clang
clang++              13.0.1              /usr/bin/clang++
llvm-ar              unknown             /usr/bin/llvm-ar
llvm-lib             unknown             /usr/bin/llvm-lib
lld                  13.0.1              /usr/bin/lld
lld-link             13.0.1              /usr/bin/lld-link
lldb                 13.0.1              /usr/bin/lldb
lldb-server          unknown             /usr/bin/lldb-server

----------------------------misc----------------------------
cargo                1.60.0              /usr/bin/cargo
git                  2.35.1              /usr/bin/git
flutter              2.10.4              /usr/bin/flutter

--------------------------android---------------------------
adb                  1.0.41              /opt/android-sdk/platform-tools/adb
javac                11.0.15             /usr/bin/javac
java                 11.0.15             /usr/bin/java

----------------------------ios-----------------------------
idevice_id           1.3.0-83-g5b8c9a8   /usr/bin/idevice_id
ideviceinfo          1.3.0-83-g5b8c9a8   /usr/bin/ideviceinfo
ideviceinstaller     1.1.1               /usr/bin/ideviceinstaller
ideviceimagemounter  1.3.0-83-g5b8c9a8   /usr/bin/ideviceimagemounter
idevicedebug         1.3.0-83-g5b8c9a8   /usr/bin/idevicedebug

---------------------------linux----------------------------
mksquashfs           4.5.1               /usr/bin/mksquashfs

---------------------------macos----------------------------
hdiutil              not found
```

### error: failed to run custom build command for glib-sys v0.14.0
This means that `gtk3-dev` is not installed. Install `gtk3-dev` package to fix the problem.

### No devices found with name or id matching 'linux'.
Run `flutter config --enable-linux-desktop`.

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
