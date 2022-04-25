use crate::flutter::Flutter;
use crate::{BuildEnv, CompileTarget};
use anyhow::Result;
use std::fs::File;
use std::path::Path;
use std::process::Command;

pub fn build_ios_main(
    env: &BuildEnv,
    flutter: &Flutter,
    target: CompileTarget,
    lib: Option<&Path>,
) -> Result<()> {
    let arch_dir = env.arch_dir(target.arch());
    let sdk = env.ios_sdk();
    let main_m = arch_dir.join("main.m");
    let main = arch_dir.join("main");
    let framework = flutter
        .engine_dir(target)?
        .join("Flutter.xcframework")
        .join("ios-arm64_armv7");
    std::fs::write(&main_m, include_bytes!("../../assets/main.m"))?;
    let mut clang = Command::new("clang");
    clang
        .env("SDKROOT", &sdk)
        .arg("-objc")
        .arg("-fmodules")
        .arg("--target=arm64-apple-ios")
        .arg("-miphoneos-version-min=9.0")
        .arg(format!("--sysroot={}", sdk.display()))
        .arg("-F")
        .arg(framework)
        .arg("-framework")
        .arg("Flutter")
        .arg("-rpath")
        .arg("@executable_path/Frameworks")
        .arg("-fuse-ld=lld")
        .arg("-o")
        .arg(&main)
        .arg(&main_m);
    if let Some(lib) = lib {
        clang.arg("-v").arg("-Wl,-force_load").arg(lib);
    }
    anyhow::ensure!(clang.status()?.success(), "failed to build main.m");
    Ok(())
}

pub fn build_empty_dylib(env: &BuildEnv, target: CompileTarget) -> Result<()> {
    let arch_dir = env.arch_dir(target.arch());
    let sdk = env.ios_sdk();
    let app_m = arch_dir.join("App.m");
    let app = arch_dir.join("App");
    File::create(&app_m)?;
    let status = Command::new("clang")
        .env("SDKROOT", &sdk)
        .arg("--target=arm64-apple-ios")
        .arg("-miphoneos-version-min=9.0")
        .arg(format!("--sysroot={}", sdk.display()))
        .arg("-fuse-ld=lld")
        .arg("-shared")
        .arg("-v")
        .arg("-o")
        .arg(&app)
        .arg(&app_m)
        .status()?;
    anyhow::ensure!(status.success(), "failed to build main.m");
    Ok(())
}
