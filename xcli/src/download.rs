use crate::{Arch, BuildEnv, CompileTarget, Flutter, Opt, Platform};
use anyhow::Result;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use tar::{Archive, EntryType};
use zstd::Decoder;

pub async fn download_artifacts(env: &BuildEnv) -> Result<()> {
    let mut manager = DownloadManager::new(env);
    match env.target().platform() {
        Platform::Linux if Platform::host()? != Platform::Linux => {
            anyhow::bail!("cross compiling to linux is not yet supported");
        }
        Platform::Windows if Platform::host()? != Platform::Windows => {
            manager.windows_sdk();
        }
        Platform::Macos if Platform::host()? != Platform::Macos => {
            manager.macos_sdk();
        }
        Platform::Android => {
            manager.android_ndk();
            manager.android_jar()?;
        }
        Platform::Ios if Platform::host()? != Platform::Macos => {
            manager.ios_sdk();
        }
        _ => {}
    }
    if env.flutter().is_some() {
        let host = CompileTarget::new(Platform::host()?, Arch::host()?, Opt::Debug);
        for target in env.target().compile_targets().chain(std::iter::once(host)) {
            manager.flutter_engine(target)?;
        }
        manager.material_fonts()?;
    }
    manager.complete().await
}

pub struct DownloadManager<'a> {
    env: &'a BuildEnv,
    queue: VecDeque<WorkItem>,
    client: reqwest::blocking::Client,
}

impl<'a> DownloadManager<'a> {
    pub fn new(env: &'a BuildEnv) -> Self {
        let client = reqwest::blocking::Client::new();
        Self {
            env,
            client,
            queue: Default::default(),
        }
    }

    pub(crate) fn download(&mut self, item: WorkItem) {
        if !item.output.exists() {
            self.queue.push_back(item);
        }
    }

    pub(crate) fn flutter(&self) -> Option<&Flutter> {
        self.env.flutter()
    }

    pub async fn complete(mut self) -> Result<()> {
        let download_dir = self.env.cache_dir().join("download");
        std::fs::create_dir_all(&download_dir)?;
        while let Some(item) = self.queue.pop_front() {
            let name = item.url.rsplit_once('/').unwrap().1;
            let ext = name.split_once('.').map(|x| x.1);
            let result = (|| {
                let mut resp = self.client.get(&item.url).send()?;
                if !resp.status().is_success() {
                    anyhow::bail!("GET {} returned status code {}", &item.url, resp.status());
                }
                if let Some(ext) = ext {
                    let archive = download_dir.join(name);
                    std::io::copy(&mut resp, &mut BufWriter::new(File::create(&archive)?))?;
                    match ext {
                        "tar.zst" => {
                            let archive = BufReader::new(File::open(&archive)?);
                            let mut archive = Archive::new(Decoder::new(archive)?);
                            for entry in archive.entries()? {
                                let mut entry = entry?;
                                if item.no_symlinks
                                    && entry.header().entry_type() == EntryType::Symlink
                                {
                                    continue;
                                }
                                if item.no_colons
                                    && entry.header().path()?.to_str().unwrap().contains(':')
                                {
                                    continue;
                                }
                                entry.unpack(&item.output)?;
                            }
                        }
                        "framework.zip" => {
                            let framework_dir = download_dir.join("framework");
                            xcommon::extract_zip(&archive, &framework_dir)?;
                            let archive = framework_dir.join(name);
                            xcommon::extract_zip(&archive, &item.output)?;
                        }
                        "zip" => {
                            xcommon::extract_zip(&archive, &item.output)?;
                        }
                        _ => unimplemented!(),
                    }
                } else {
                    std::io::copy(&mut resp, &mut BufWriter::new(File::create(&item.output)?))?;
                }
                Ok(())
            })();
            if result.is_err() {
                if item.output.is_dir() {
                    std::fs::remove_dir_all(&item.output).ok();
                } else {
                    std::fs::remove_file(&item.output).ok();
                }
            }
        }
        Ok(())
    }
}

pub struct WorkItem {
    url: String,
    output: PathBuf,
    no_symlinks: bool,
    no_colons: bool,
}

impl WorkItem {
    pub fn new(output: PathBuf, url: String) -> Self {
        Self {
            url,
            output,
            no_symlinks: false,
            no_colons: false,
        }
    }

    /// The windows sdk contains symlinks for case sensitive
    /// filesystems. on case sensitive file systems skip the
    /// symlinks
    pub fn no_symlinks(&mut self) -> &mut Self {
        self.no_symlinks = true;
        self
    }

    /// the macos sdk contains man pages. man pages contain
    /// colons in the file names. on windows it's an invalid
    /// file name character, so we skip file names with colons.
    pub fn no_colons(&mut self) -> &mut Self {
        self.no_colons = true;
        self
    }
}

impl WorkItem {
    pub fn github_release(
        output: PathBuf,
        org: &str,
        name: &str,
        version: &str,
        artifact: &str,
    ) -> Self {
        Self::new(
            output,
            format!(
                "https://github.com/{}/{}/releases/download/{}/{}",
                org, name, version, artifact
            ),
        )
    }
}

impl<'a> DownloadManager<'a> {
    pub fn android_jar(&self) -> Result<PathBuf> {
        let dir = self.env.android_sdk();
        let sdk = self.env.target_sdk_version();
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

    pub fn windows_sdk(&mut self) -> PathBuf {
        let output = self.env.windows_sdk();
        let mut item = WorkItem::github_release(
            output.clone(),
            "cloudpeer",
            "x",
            "v0.1.0+2",
            "Windows.sdk.tar.zst",
        );
        if !cfg!(target_os = "linux") {
            item.no_symlinks();
        }
        self.download(item);
        output
    }

    pub fn macos_sdk(&mut self) -> PathBuf {
        let output = self.env.macos_sdk();
        let mut item = WorkItem::github_release(
            output.clone(),
            "cloudpeer",
            "x",
            "v0.1.0+2",
            "MacOSX.sdk.tar.zst",
        );
        if cfg!(target_os = "windows") {
            item.no_colons();
        }
        self.download(item);
        output
    }

    pub fn android_ndk(&mut self) -> PathBuf {
        let output = self.env.android_ndk();
        let item = WorkItem::github_release(
            output.clone(),
            "cloudpeer",
            "x",
            "v0.1.0+2",
            "Android.ndk.tar.zst",
        );
        self.download(item);
        output
    }

    pub fn ios_sdk(&mut self) -> PathBuf {
        let output = self.env.ios_sdk();
        let mut item = WorkItem::github_release(
            output.clone(),
            "cloudpeer",
            "x",
            "v0.1.0+2",
            "iPhoneOS.sdk.tar.zst",
        );
        if cfg!(target_os = "windows") {
            item.no_colons();
        }
        self.download(item);
        output
    }
}
