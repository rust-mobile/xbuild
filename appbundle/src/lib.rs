use anyhow::Result;
use apple_codesign::{BundleSigner, ExecutableSegmentFlags, SettingsScope, SigningSettings};
use icns::{IconFamily, Image};
use pkcs8::ToPrivateKey;
use plist::Value;
use rasn_cms::{ContentInfo, SignedData};
use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::{Path, PathBuf};
use std::process::Command;
use x509_certificate::{CapturedX509Certificate, InMemorySigningKeyPair};
use xcommon::{Scaler, ScalerOpts, Signer};

mod info;

pub use info::InfoPlist;

const ICON_SIZES: [u32; 6] = [16, 32, 64, 128, 256, 512];

pub struct AppBundle {
    appdir: PathBuf,
    info: InfoPlist,
    entitlements: Option<Value>,
    team_id: Option<String>,
}

impl AppBundle {
    pub fn new(build_dir: &Path, info: InfoPlist) -> Result<Self> {
        if info.name.is_none() {
            anyhow::bail!("missing info.name");
        }
        let appdir = build_dir.join(format!("{}.app", info.name.as_ref().unwrap()));
        std::fs::remove_dir_all(&appdir).ok();
        std::fs::create_dir_all(&appdir)?;
        Ok(Self {
            appdir,
            info,
            entitlements: None,
            team_id: None,
        })
    }

    pub fn appdir(&self) -> &Path {
        &self.appdir
    }

    fn content_dir(&self) -> PathBuf {
        if self.info.requires_ios == Some(true) {
            self.appdir.to_path_buf()
        } else {
            self.appdir.join("Contents")
        }
    }

    fn resource_dir(&self) -> PathBuf {
        if self.info.requires_ios == Some(true) {
            self.content_dir()
        } else {
            self.content_dir().join("Resources")
        }
    }

    fn framework_dir(&self) -> PathBuf {
        self.content_dir().join("Frameworks")
    }

    fn executable_dir(&self) -> PathBuf {
        let contents = self.content_dir();
        if self.info.requires_ios == Some(true) {
            contents
        } else {
            contents.join("MacOS")
        }
    }

    pub fn add_icon(&mut self, path: &Path) -> Result<()> {
        let mut icns = IconFamily::new();
        let scaler = Scaler::open(path)?;
        let mut buf = vec![];
        for size in ICON_SIZES {
            buf.clear();
            let mut cursor = Cursor::new(&mut buf);
            scaler.write(&mut cursor, ScalerOpts::new(size))?;
            let image = Image::read_png(&*buf)?;
            icns.add_icon(&image)?;
        }
        let path = self.resource_dir().join("AppIcon.icns");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        icns.write(BufWriter::new(File::create(path)?))?;
        self.info.icon_file = Some("AppIcon".to_string());
        Ok(())
    }

    pub fn add_file(&self, path: &Path, dest: &Path) -> Result<()> {
        let dest = self.resource_dir().join(dest);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, dest)?;
        Ok(())
    }

    pub fn add_directory(&self, source: &Path, dest: &Path) -> Result<()> {
        let resource_dir = self.resource_dir().join(dest);
        std::fs::create_dir_all(&resource_dir)?;
        xcommon::copy_dir_all(source, &resource_dir)?;
        Ok(())
    }

    pub fn add_framework_directory(&self, framework: &str, source: &Path, dest: &Path) -> Result<()> {
        let framework = self.framework_dir().join(framework);
        let resource_dir = if self.info.requires_ios == Some(true) {
            framework
        } else {
            framework.join("Resources")
        };
        let dest = resource_dir.join(dest);
        std::fs::create_dir_all(&dest)?;
        xcommon::copy_dir_all(source, &dest)?;
        Ok(())
    }

    pub fn add_executable(&mut self, path: &Path) -> Result<()> {
        let file_name = path.file_name().unwrap().to_str().unwrap();
        let exe_dir = self.executable_dir();
        std::fs::create_dir_all(&exe_dir)?;
        std::fs::copy(path, exe_dir.join(file_name))?;
        if self.info.executable.is_none() {
            self.info.executable = Some(file_name.to_string());
        }
        Ok(())
    }

    pub fn add_framework(&self, path: &Path) -> Result<()> {
        let framework_dir = self.framework_dir().join(path.file_name().unwrap());
        std::fs::create_dir_all(&framework_dir)?;
        xcommon::copy_dir_all(path, &framework_dir)?;
        Ok(())
    }

    pub fn add_lib(&self, path: &Path) -> Result<()> {
        let file_name = path.file_name().unwrap();
        let framework_dir = self.framework_dir();
        std::fs::create_dir_all(&framework_dir)?;
        std::fs::copy(path, framework_dir.join(file_name))?;
        Ok(())
    }

    pub fn add_provisioning_profile(&mut self, path: &Path) -> Result<()> {
        let cms = std::fs::read(path)?;
        let info =
            rasn::der::decode::<ContentInfo>(&cms).map_err(|err| anyhow::anyhow!("{}", err))?;
        let data = rasn::der::decode::<SignedData>(info.content.as_bytes())
            .map_err(|err| anyhow::anyhow!("{}", err))?;
        let xml = data.encap_content_info.content.as_ref().unwrap().as_ref();
        let profile: plist::Value = plist::from_reader_xml(xml)?;
        let dict = profile
            .as_dictionary()
            .ok_or_else(|| anyhow::anyhow!("invalid provisioning profile"))?;
        let entitlements = dict
            .get("Entitlements")
            .ok_or_else(|| anyhow::anyhow!("missing key Entitlements"))?
            .clone();
        let team_id = dict
            .get("TeamIdentifier")
            .ok_or_else(|| anyhow::anyhow!("missing key TeamIdentifier"))?
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("missing team identifier"))?
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("missing team identifier"))?
            .as_string()
            .ok_or_else(|| anyhow::anyhow!("missing team identifier"))?
            .to_string();
        self.entitlements = Some(entitlements);
        self.team_id = Some(team_id);
        std::fs::copy(path, self.appdir().join("embedded.mobileprovision"))?;
        Ok(())
    }

    pub fn finish(self, signer: Option<Signer>) -> Result<PathBuf> {
        let path = self.content_dir().join("Info.plist");
        plist::to_file_xml(path, &self.info)?;

        if let Some(signer) = signer {
            let mut signing_settings = SigningSettings::default();
            signing_settings.set_executable_segment_flags(
                SettingsScope::Main,
                ExecutableSegmentFlags::MAIN_BINARY,
            );
            let cert =
                CapturedX509Certificate::from_der(rasn::der::encode(signer.cert()).unwrap())?;
            let key = InMemorySigningKeyPair::from_pkcs8_der(signer.key().to_pkcs8_der().unwrap())?;
            signing_settings.set_signing_key(&key, cert);
            signing_settings.chain_apple_certificates();
            if let Some(entitlements) = self.entitlements.as_ref() {
                let mut buf = vec![];
                entitlements.to_writer_xml(&mut buf)?;
                let entitlements = std::str::from_utf8(&buf)?;
                signing_settings.set_entitlements_xml(SettingsScope::Main, entitlements)?;
            }
            if let Some(team_id) = self.team_id.as_ref() {
                signing_settings.set_team_id(team_id)
            }
            let bundle_signer = BundleSigner::new_from_path(self.appdir())?;
            bundle_signer.write_signed_bundle(self.appdir(), &signing_settings)?;
        }

        Ok(self.appdir)
    }
}

pub fn make_dmg(build_dir: &Path, appbundle: &Path, dmg: &Path) -> Result<()> {
    let name = dmg.file_stem().unwrap().to_str().unwrap();
    let uncompressed = build_dir.join(format!("{}.uncompressed.dmg", name));
    make_uncompressed_dmg(appbundle, &uncompressed, name)?;
    make_compressed_dmg(&uncompressed, dmg)?;
    Ok(())
}

fn make_uncompressed_dmg(appbundle: &Path, uncompressed_dmg: &Path, volname: &str) -> Result<()> {
    let status = Command::new("hdiutil")
        .arg("create")
        .arg(uncompressed_dmg)
        .arg("-ov")
        .arg("-volname")
        .arg(volname)
        .arg("-fs")
        .arg("HFS+")
        .arg("-srcfolder")
        .arg(appbundle)
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to build uncompressed dmg");
    }
    Ok(())
}

fn make_compressed_dmg(uncompressed_dmg: &Path, dmg: &Path) -> Result<()> {
    let status = Command::new("hdiutil")
        .arg("convert")
        .arg(uncompressed_dmg)
        .arg("-format")
        .arg("UDZO")
        .arg("-o")
        .arg(dmg)
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to build compressed dmg");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobile_provision() -> Result<()> {
        let mut bundle = AppBundle::new(Path::new("/tmp").into(), InfoPlist::default())?;
        bundle.add_provisioning_profile(Path::new("/home/dvc/embedded.mobileprovision"))?;
        Ok(())
    }
}
