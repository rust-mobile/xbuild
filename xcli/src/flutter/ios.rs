use crate::flutter::Flutter;
use crate::{BuildEnv, CompileTarget};
use anyhow::Result;
use std::process::Command;

pub fn build_ios_main(env: &BuildEnv, flutter: &Flutter, target: CompileTarget) -> Result<()> {
    let arch_dir = env.arch_dir(target.arch());
    let sdk = env.ios_sdk();
    let main_m = arch_dir.join("main.m");
    let main = arch_dir.join("main");
    let framework = flutter
        .engine_dir(target)?
        .join("Flutter.xcframework")
        .join("ios-arm64_armv7");
    std::fs::write(&main_m, include_bytes!("../../assets/main.m"))?;
    let status = Command::new("clang")
        .arg("-objc")
        .arg("-fmodules")
        .arg("--target=arm64-apple-ios")
        .arg(format!("--sysroot={}", sdk.display()))
        .arg("-F")
        .arg(framework)
        .arg("-fuse-ld=lld")
        .arg("-o")
        .arg(&main)
        .arg(&main_m)
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to build main.m");
    }
    Ok(())
}
