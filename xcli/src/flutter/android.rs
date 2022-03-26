use crate::{task, BuildEnv, Opt};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_classes_dex(env: &BuildEnv, r8: &Path, deps: Vec<PathBuf>) -> Result<()> {
    let platform_dir = env.platform_dir();
    let android_jar = env.android_jar();

    // build GeneratedPluginRegistrant
    let plugins = platform_dir.join("GeneratedPluginRegistrant.java");
    std::fs::write(
        &plugins,
        include_bytes!("../../assets/GeneratedPluginRegistrant.java"),
    )?;
    let separator = if cfg!(windows) { ";" } else { ":" };
    let classpath = deps
        .iter()
        .chain(std::iter::once(&android_jar))
        .map(|d| d.display().to_string())
        .collect::<Vec<_>>()
        .join(separator);
    let java = platform_dir.join("java");
    let mut cmd = Command::new("javac");
    cmd.arg("--class-path")
        .arg(classpath)
        .arg(plugins)
        .arg("-d")
        .arg(&java);
    task::run(cmd, env.verbose())?;

    // build classes.dex
    let pg = platform_dir.join("proguard-rules.pro");
    std::fs::write(&pg, include_bytes!("../../assets/proguard-rules.pro"))?;
    let plugins = java
        .join("io")
        .join("flutter")
        .join("plugins")
        .join("GeneratedPluginRegistrant.class");
    let mut java = Command::new("java");
    java.arg("-cp")
        .arg(r8)
        .arg("com.android.tools.r8.R8")
        .args(deps)
        .arg(plugins)
        .arg("--lib")
        .arg(android_jar)
        .arg("--output")
        .arg(platform_dir)
        .arg("--pg-conf")
        .arg(pg);
    if env.target().opt() == Opt::Release {
        java.arg("--release");
    }
    task::run(java, env.verbose())?;
    Ok(())
}
