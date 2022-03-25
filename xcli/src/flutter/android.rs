use crate::flutter::Flutter;
use crate::{BuildEnv, Opt};
use anyhow::Result;
use maven::{Maven, Package, Version};
use std::path::Path;
use std::process::Command;
use xapk::Target;

const GOOGLE: &'static str = "https://maven.google.com";
const FLUTTER: &'static str = "http://download.flutter.io";
const CENTRAL: &'static str = "https://repo1.maven.org/maven2";

pub fn maven(dir: &Path) -> Result<Maven> {
    let mut maven = Maven::new(dir.join("maven"))?;
    maven.add_repository(GOOGLE);
    maven.add_repository(FLUTTER);
    maven.add_repository(CENTRAL);
    Ok(maven)
}

pub fn build_classes_dex(env: &BuildEnv, flutter: &Flutter) -> Result<()> {
    let maven = maven(env.build_dir())?;
    let platform_dir = env.platform_dir();
    let engine_version = flutter.engine_version()?;
    let android_jar = env.android_jar();
    let flutter_embedding = FlutterEmbedding::new(env.target().opt(), &engine_version);
    let deps = maven
        .resolve(flutter_embedding.package(), flutter_embedding.version())?
        .into_iter()
        .filter(|path| {
            path.extension() == Some("jar".as_ref()) || path.extension() == Some("aar".as_ref())
        })
        .collect::<Vec<_>>();
    let r8 = R8::new(3, 1, 51);
    let r8 = maven.package(&r8.package(), &r8.version())?;

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
    let status = Command::new("javac")
        .arg("--class-path")
        .arg(classpath)
        .arg(plugins)
        .arg("-d")
        .arg(&java)
        .status()?;
    if !status.success() {
        anyhow::bail!("javac exited with nonzero exit code.");
    }

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
    if !java.status()?.success() {
        anyhow::bail!("`{:?}` exited with nonzero exit code.", java);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct R8 {
    major: u32,
    minor: u32,
    patch: u32,
}

impl R8 {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn package(self) -> Package {
        Package {
            group: "com.android.tools".into(),
            name: "r8".into(),
        }
    }

    pub fn version(self) -> Version {
        Version {
            major: self.major,
            minor: self.minor,
            patch: self.patch,
            suffix: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FlutterEmbedding<'a> {
    opt: Opt,
    engine_version: &'a str,
}

impl<'a> FlutterEmbedding<'a> {
    pub fn new(opt: Opt, engine_version: &'a str) -> Self {
        Self {
            opt,
            engine_version,
        }
    }

    pub fn package(self) -> Package {
        Package {
            group: "io.flutter".into(),
            name: format!("flutter_embedding_{}", self.opt),
        }
    }

    pub fn version(self) -> Version {
        Version {
            major: 1,
            minor: 0,
            patch: 0,
            suffix: Some(self.engine_version.into()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FlutterEngine<'a> {
    target: Target,
    opt: Opt,
    engine_version: &'a str,
}

impl<'a> FlutterEngine<'a> {
    pub fn new(target: Target, opt: Opt, engine_version: &'a str) -> Self {
        Self {
            target,
            opt,
            engine_version,
        }
    }

    pub fn package(self) -> Package {
        let target = match self.target {
            Target::Arm64V8a => "arm64_v8a",
            Target::ArmV7a => "armeabi_v7a",
            Target::X86 => "x86",
            Target::X86_64 => "x86_64",
        };
        Package {
            group: "io.flutter".into(),
            name: format!("{}_{}", target, self.opt),
        }
    }

    pub fn version(self) -> Version {
        Version {
            major: 1,
            minor: 0,
            patch: 0,
            suffix: Some(self.engine_version.into()),
        }
    }
}
