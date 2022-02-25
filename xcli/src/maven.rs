use crate::Opt;
use maven::{Package, Version};
use xapk::Target;

pub use maven::Maven;

pub const GOOGLE: &'static str = "https://maven.google.com";
pub const FLUTTER: &'static str = "http://download.flutter.io";
pub const CENTRAL: &'static str = "https://repo1.maven.org/maven2";

#[derive(Clone, Copy, Debug)]
pub struct FlutterEmbedding<'a> {
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
