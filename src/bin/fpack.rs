use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
use std::process::Command;
use xstore::apk::{ApkBuilder, Xml};
use xstore::appimage::AppImageBuilder;
use xstore::{Signer, ZipFileOptions};
use zip::read::ZipArchive;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    format: String,
    #[clap(short, long)]
    debug: bool,
    #[clap(short, long)]
    key: String,
    #[clap(short, long)]
    cert: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Pubspec {
    name: String,
    description: String,
    version: String,
    appimage_config: Option<AppimageConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppimageConfig {
    logo_path: String,
}

fn flutter_build(target: &str, debug: bool) -> Result<()> {
    let mut cmd = Command::new("flutter");
    cmd.arg("build").arg(target);
    if debug {
        cmd.arg("--debug");
    }
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("failed to run flutter");
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let f = File::open("pubspec.yaml")?;
    let pubspec: Pubspec = serde_yaml::from_reader(f)?;
    let key = std::fs::read_to_string(&args.key)?;
    let cert = std::fs::read_to_string(&args.cert)?;
    let signer = Signer::new(&key, &cert)?;
    match args.format.as_str() {
        "appimage" => {
            flutter_build("linux", args.debug)?;
            let build_dir = Path::new("build").join("linux").join("x64");
            let build_dir = if args.debug {
                build_dir.join("debug")
            } else {
                build_dir.join("release")
            };
            let builder = AppImageBuilder::new(&build_dir, pubspec.name)?;
            builder.add_directory(&build_dir.join("bundle"), None)?;
            builder.add_apprun()?;
            builder.add_desktop()?;
            if let Some(config) = pubspec.appimage_config.as_ref() {
                builder.add_icon(config.logo_path.as_ref())?;
            }
            builder.sign(&signer)?;
        }
        "apk" => {
            flutter_build("apk", args.debug)?;
            let intermediates = Path::new("build").join("app").join("intermediates");
            let opt = if args.debug { "debug" } else { "release" };
            let mut apk = File::create(format!("{}.apk", &pubspec.name))?;
            let mut builder = ApkBuilder::new(&mut apk);
            let assets = intermediates.join("merged_assets").join(opt).join("out");
            builder.add_directory(&assets, Some(Path::new("assets")))?;
            let libs = intermediates
                .join("merged_native_libs")
                .join(opt)
                .join("out");
            builder.add_directory(&libs, None)?;
            let classes = intermediates
                .join("dex")
                .join(opt)
                .join("mergeDexDebug")
                .join("classes.dex");
            builder.add_file(&classes, "classes.dex", ZipFileOptions::Compressed)?;
            let manifest = intermediates
                .join("merged_manifest")
                .join(opt)
                .join("out")
                .join("AndroidManifest.xml");
            builder.add_manifest(&Xml::from_path(&manifest)?)?;
            // TODO: generate resources*/
            /*let apk = Path::new("build")
                .join("app")
                .join("outputs")
                .join("apk")
                .join(opt)
                .join(format!("app-{}.apk", opt));
            let mut f = File::open(apk)?;
            let mut zip = ZipArchive::new(&mut f)?;
            let mut file_names = vec![];
            for name in zip.file_names() {
                if name.starts_with("res") || name == "resources.arsc" {
                    file_names.push(name.to_string());
                }
            }
            for name in file_names {
                let f = zip.by_name(&name)?;
                builder.add_file_from_archive(f)?;
            }*/
            builder.sign(&signer)?;
        }
        "aab" => {
            flutter_build("appbundle", args.debug)?;
        }
        "msix" => {
            // TODO
            panic!("msix");
        }
        format => anyhow::bail!("unsupported format {}", format),
    }
    Ok(())
}
