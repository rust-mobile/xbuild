use anyhow::Result;
use std::path::Path;

fn cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[build-dependencies]
ffi-gen = "0.1.13"

[dependencies]
anyhow = "1.0.56"
ffi-gen-macro = "0.1.2"
futures = "0.3.21"

[target.'cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))'.dependencies]
env_logger = "0.9.0"
nativeshell = {{ git = "https://github.com/nativeshell/nativeshell" }}
"#,
        name = name,
    )
}

fn pubspec_yaml(name: &str) -> String {
    format!(
        r#"name: {name}
version: 0.1.0

environment:
  sdk: '>2.15.1 <3.0.0'

dependencies:
  flutter:
    sdk: flutter
  nativeshell: ^0.1.13

flutter:
  uses-material-design: true
"#,
        name = name
    )
}

fn build_rs(name: &str) -> String {
    format!(
        r#"use ffi_gen::FfiGen;
use std::path::PathBuf;

fn main() {{
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let api = dir.join("api.rsh");
    println!(
        "cargo:rerun-if-changed={{}}",
        api.as_path().to_str().unwrap(),
    );
    let ffigen = FfiGen::new(&api).unwrap();
    let bindings = dir.join("lib").join("bindings.dart");
    ffigen.generate_dart(bindings, "{name}", "{name}").unwrap();
}}
"#,
        name = name,
    )
}

pub fn new(name: &str) -> Result<()> {
    let root = Path::new(name);
    let src = root.join("src");
    let lib = root.join("lib");
    std::fs::create_dir(&root)?;
    std::fs::create_dir(&src)?;
    std::fs::create_dir(&lib)?;
    std::fs::write(root.join("Cargo.toml"), cargo_toml(name))?;
    std::fs::write(root.join("pubspec.yaml"), pubspec_yaml(name))?;
    std::fs::write(root.join("build.rs"), build_rs(name))?;
    std::fs::write(
        root.join(".gitignore"),
        include_bytes!("../../assets/template/.gitignore"),
    )?;
    std::fs::write(
        root.join("rust-toolchain.toml"),
        include_bytes!("../../assets/template/rust-toolchain.toml"),
    )?;
    std::fs::write(
        root.join("api.rsh"),
        include_bytes!("../../assets/template/api.rsh"),
    )?;
    std::fs::write(
        src.join("lib.rs"),
        include_bytes!("../../assets/template/lib.rs"),
    )?;
    std::fs::write(
        src.join("main.rs"),
        include_bytes!("../../assets/template/main.rs"),
    )?;
    std::fs::write(
        lib.join("main.dart"),
        include_bytes!("../../assets/template/main.dart"),
    )?;
    Ok(())
}
