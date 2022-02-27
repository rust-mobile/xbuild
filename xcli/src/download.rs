use crate::{Arch, CompileTarget, Opt, Platform};
use anyhow::Result;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use tar::{Archive, Builder, EntryType};
use zstd::Decoder;

pub fn github_release_tar_zst(
    out: &Path,
    org: &str,
    repo: &str,
    version: &str,
    artifact: &str,
    no_symlinks: bool,
) -> Result<()> {
    let url = format!(
        "https://github.com/{}/{}/releases/download/{}/{}",
        org, repo, version, artifact
    );
    let client = reqwest::blocking::Client::new();
    let resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("GET {} returned status code {}", url, resp.status());
    }
    let mut archive = Archive::new(Decoder::new(resp)?);
    if no_symlinks {
        let mut buf = vec![];
        let mut builder = Builder::new(&mut buf);
        for entry in archive.entries()? {
            let entry = entry?;
            if entry.header().entry_type() != EntryType::Symlink {
                builder.append_entry(entry)?;
            }
        }
        builder.into_inner()?;
        Archive::new(&*buf).unpack(out)?;
    } else {
        archive.unpack(out)?;
    }
    Ok(())
}

pub fn flutter_engine(engine_dir: &Path, engine: &str, target: CompileTarget) -> Result<()> {
    let mut artifacts = Vec::with_capacity(4);
    match (target.platform(), target.arch(), target.opt()) {
        (Platform::Linux, arch, Opt::Debug) => {
            artifacts.push(format!("linux-{}/artifacts.zip", arch));
            artifacts.push(format!(
                "linux-{arch}/linux-{arch}-flutter-gtk.zip",
                arch = arch
            ));
        }
        (Platform::Linux, arch, Opt::Release) => {
            artifacts.push(format!("linux-{}/artifacts.zip", arch));
            artifacts.push(format!(
                "linux-{arch}-release/linux-{arch}-flutter-gtk.zip",
                arch = arch
            ));
        }
        (Platform::Macos, arch, Opt::Debug) => {
            artifacts.push(format!("darwin-{}/artifacts.zip", arch));
            artifacts.push(format!("darwin-{}/FlutterMacOS.framework.zip", arch));
        }
        (Platform::Macos, arch, Opt::Release) => {
            artifacts.push(format!("darwin-{}/artifacts.zip", arch));
            artifacts.push(format!("darwin-{}/FlutterMacOS.framework.zip", arch));
        }
        (Platform::Windows, arch, Opt::Debug) => {
            artifacts.push(format!(
                "windows-{arch}/windows-{arch}-flutter.zip",
                arch = arch
            ));
        }
        (Platform::Windows, arch, Opt::Release) => {
            artifacts.push(format!(
                "windows-{arch}-release/windows-{arch}-flutter.zip",
                arch = arch
            ));
        }
        (Platform::Android, arch, Opt::Debug) => {
            artifacts.push(format!("android-{}/artifacts.zip", arch));
        }
        (Platform::Android, arch, Opt::Release) => {
            artifacts.push(format!("android-{}-release/artifacts.zip", arch));
            let host = Platform::host()?;
            let platform = if host == Platform::Macos {
                "darwin".to_string()
            } else {
                host.to_string()
            };
            artifacts.push(format!(
                "android-{}-release/{}-{}.zip",
                arch,
                &platform,
                Arch::host()?
            ));
        }
        (Platform::Ios, _, Opt::Debug) => {
            artifacts.push("ios/artifacts.zip".to_string());
        }
        (Platform::Ios, _, Opt::Release) => {
            artifacts.push("ios-release/artifacts.zip".to_string());
        }
    }
    for artifact in &artifacts {
        if let Err(err) = download_flutter_artifact(engine_dir, engine, artifact) {
            std::fs::remove_dir_all(engine_dir).ok();
            return Err(err);
        }
        if artifact.ends_with(".framework.zip") {
            let file_name = Path::new(artifact).file_name().unwrap().to_str().unwrap();
            let archive = engine_dir.join(file_name);
            let framework = engine_dir.join(file_name.strip_suffix(".zip").unwrap());
            std::fs::create_dir(&framework)?;
            xcommon::extract_zip(&archive, &framework)?;
        }
    }
    Ok(())
}

fn download_flutter_artifact(dir: &Path, version: &str, artifact: &str) -> Result<()> {
    let file_name = Path::new(artifact).file_name().unwrap();
    let path = dir.join("download").join(file_name);
    let url = format!(
        "https://storage.googleapis.com/flutter_infra_release/flutter/{}/{}",
        version, artifact
    );
    let client = reqwest::blocking::Client::new();
    let mut resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("GET {} returned status code {}", url, resp.status());
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut f = BufWriter::new(File::create(&path)?);
    std::io::copy(&mut resp, &mut f)?;
    xcommon::extract_zip(&path, dir)?;
    Ok(())
}
