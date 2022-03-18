use crate::{Arch, CompileTarget, Opt, Platform};
use anyhow::Result;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use tar::{Archive, Builder, EntryType};
use zstd::Decoder;

/// Unpacks a github archive with some compatibility options.
///
/// no_symlinks:
///   the windows sdk contains symlinks for case sensitive
///   filesystems. on case sensitive file systems skip the
///   symlinks
///
/// no_colons:
///   the macos sdk contains man pages. man pages contain
///   colons in the file names. on windows it's an invalid
///   file name character, so we skip file names with colons.
pub fn github_release_tar_zst(
    out: &Path,
    org: &str,
    repo: &str,
    version: &str,
    artifact: &str,
    no_symlinks: bool,
    no_colons: bool,
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
    if no_symlinks || no_colons {
        let mut buf = vec![];
        let mut builder = Builder::new(&mut buf);
        for entry in archive.entries()? {
            let entry = entry?;
            if no_symlinks && entry.header().entry_type() == EntryType::Symlink {
                continue;
            }
            if no_colons && entry.header().path()?.to_str().unwrap().contains(':') {
                continue;
            }
            builder.append_entry(entry)?;
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
    if target.platform() == Platform::host()?
        && target.arch() == Arch::host()?
        && target.opt() == Opt::Debug
    {
        artifacts.push("flutter_patched_sdk.zip".to_string());
        artifacts.push("flutter_patched_sdk_product.zip".to_string());
        if target.platform() != Platform::Macos {
            artifacts.push(format!(
                "{}-{}/artifacts.zip",
                target.platform(),
                target.arch()
            ));
        } else {
            artifacts.push(format!("darwin-{}/artifacts.zip", target.arch()));
        }
    }
    match (target.platform(), target.arch(), target.opt()) {
        (Platform::Linux, arch, Opt::Debug) => {
            artifacts.push(format!(
                "linux-{arch}/linux-{arch}-flutter-gtk.zip",
                arch = arch
            ));
        }
        (Platform::Linux, arch, Opt::Release) => {
            artifacts.push(format!(
                "linux-{arch}-release/linux-{arch}-flutter-gtk.zip",
                arch = arch
            ));
        }
        (Platform::Macos, arch, Opt::Debug) => {
            artifacts.push(format!("darwin-{}/FlutterMacOS.framework.zip", arch));
        }
        (Platform::Macos, arch, Opt::Release) => {
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
        (Platform::Android, _, Opt::Debug) => {}
        (Platform::Android, arch, Opt::Release) => {
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

pub fn android_jar(dir: &Path, sdk: u32) -> Result<PathBuf> {
    let path = dir
        .join("platforms")
        .join(format!("android-{}", sdk))
        .join("android.jar");
    if !path.exists() {
        let package = format!("platforms;android-{}", sdk);
        android_sdkmanager::download_and_extract_packages(
            dir.to_str().unwrap(),
            android_sdkmanager::HostOs::Linux,
            &[&package],
            Some(&[android_sdkmanager::MatchType::EntireName("android.jar")]),
        )
    }
    Ok(path)
}
