# Cross compile rust to any platform

## Obtaining and extracting the sdks.

### Android

Download https://dl.google.com/android/repository/android-ndk-r23b-linux.zip and unzip the archive. Copy
`android-ndk-r23b-linux/toolchains/llvm/prebuilt/linux-x86_64` to `~/xcross/android`. As a workaround
for std being compiled against an old ndk you'll need to create a linker script:

`echo "INPUT(-lunwind)" > ~/xcross/android/lib64/clang/12.0.8/lib/linux/aarch64/libgcc.a`

### MacOS and iOS

Go to https://developer.apple.com/download (you need to sign in with your apple id) and download
Xcode_13.2.1.xip.
Extract with `xar -xf Xcode_13.2.1.xip -C tmpdir` and `pbzx -n tmpdir/Content | cpio -i`. The
macos sdk is located in `tmpdir/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs`
and the ios sdk is located in `tmpdir/Xcode.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs`.
Copy those into `~/xcross/ios/` and `~/xcross/macos`.

### Windows

Install xwin with `cargo install xwin` and run `xwin splat --output xwin`. Move the result into `~/xcross/windows`.

### Disclaimer github releases

To cross compile at cloudpeer we've automated the process and produce releases. However this is only intended
for cloudpeer employees. Please perform the steps yourself and accept all the license agreements. Thank you.

## Configuring your ~/.cargo/config

## Using cargo-bundle
