use anyhow::{Context, Result};
use apple_codesign::app_store_connect::notary_api::SubmissionResponseStatus;
use apple_codesign::dmg::DmgSigner;
use apple_codesign::stapling::Stapler;
use apple_codesign::{
    BundleSigner, CodeSignatureFlags, NotarizationUpload, Notarizer, SettingsScope, SigningSettings,
};
use icns::{IconFamily, Image};
use pkcs8::EncodePrivateKey;
use plist::Value;
use rasn_cms::{ContentInfo, SignedData};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Cursor};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use x509_certificate::{CapturedX509Certificate, InMemorySigningKeyPair};
use xcommon::{Scaler, ScalerOpts, Signer};

mod info;

pub use info::InfoPlist;

const ICON_SIZES: [u32; 6] = [16, 32, 64, 128, 256, 512];

pub struct AppBundle {
    appdir: PathBuf,
    info: InfoPlist,
    entitlements: Option<Value>,
}

impl AppBundle {
    pub fn new(build_dir: &Path, info: InfoPlist) -> Result<Self> {
        anyhow::ensure!(info.name.is_some(), "missing info.name");
        let appdir = build_dir.join(format!("{}.app", info.name.as_ref().unwrap()));
        std::fs::remove_dir_all(&appdir).ok();
        std::fs::create_dir_all(&appdir)?;
        Ok(Self {
            appdir,
            info,
            entitlements: None,
        })
    }

    pub fn appdir(&self) -> &Path {
        &self.appdir
    }

    fn ios(&self) -> bool {
        self.info.requires_ios == Some(true)
    }

    fn content_dir(&self) -> PathBuf {
        if self.ios() {
            self.appdir.to_path_buf()
        } else {
            self.appdir.join("Contents")
        }
    }

    fn resource_dir(&self) -> PathBuf {
        if self.ios() {
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
        if self.ios() {
            contents
        } else {
            contents.join("MacOS")
        }
    }

    pub fn add_icon(&mut self, path: &Path) -> Result<()> {
        let scaler = Scaler::open(path)?;
        if self.info.requires_ios == Some(true) {
            for size in ICON_SIZES {
                let filename = format!("icon_{}x{}.png", size, size);
                let icon = self.appdir.join(&filename);
                let mut icon = BufWriter::new(File::create(icon)?);
                scaler.write(&mut icon, ScalerOpts::new(size))?;
                self.info.icon_files.push(filename);
            }
        } else {
            let mut icns = IconFamily::new();
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
        }
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

    pub fn add_provisioning_profile(&mut self, raw_profile: &[u8]) -> Result<()> {
        let info = rasn::der::decode::<ContentInfo>(raw_profile)
            .map_err(|err| anyhow::anyhow!("{}", err))?;
        let data = rasn::der::decode::<SignedData>(info.content.as_bytes())
            .map_err(|err| anyhow::anyhow!("{}", err))?;
        let xml = data.encap_content_info.content.as_ref().unwrap().as_ref();
        let profile: plist::Value = plist::from_reader_xml(xml)?;
        log::debug!("provisioning profile: {:?}", profile);
        let dict = profile
            .as_dictionary()
            .context("invalid provisioning profile")?;
        let entitlements = dict
            .get("Entitlements")
            .context("missing key Entitlements")?
            .clone();
        let app_id = entitlements
            .as_dictionary()
            .context("invalid entitlements")?
            .get("application-identifier")
            .context("missing application identifier")?
            .as_string()
            .context("missing application identifier")?;
        let bundle_prefix = app_id
            .split_once('.')
            .ok_or_else(|| anyhow::anyhow!("invalid app id {}", app_id))?
            .1;

        if let Some(bundle_identifier) = self.info.bundle_identifier.as_ref() {
            let bundle_prefix = if bundle_prefix.ends_with('*') {
                bundle_prefix.strip_suffix('*').unwrap()
            } else {
                bundle_prefix
            };
            anyhow::ensure!(
                bundle_identifier.starts_with(bundle_prefix),
                "bundle identifier mismatch"
            );
        }
        self.entitlements = Some(entitlements);
        std::fs::write(self.appdir().join("embedded.mobileprovision"), raw_profile)?;
        Ok(())
    }

    pub fn finish(&self, signer: Option<Signer>) -> Result<()> {
        let path = self.content_dir().join("Info.plist");
        plist::to_file_xml(path, &self.info)?;

        if let Some(signer) = signer {
            println!("signing {}", self.appdir().display());
            anyhow::ensure!(
                self.info.bundle_identifier.is_some(),
                "missing bundle identifier"
            );
            let mut signing_settings = SigningSettings::default();
            let cert =
                CapturedX509Certificate::from_der(rasn::der::encode(signer.cert()).unwrap())?;
            let secret = signer.key().to_pkcs8_der().unwrap();
            let key = InMemorySigningKeyPair::from_pkcs8_der(secret.as_bytes())?;
            signing_settings.set_signing_key(&key, cert);
            signing_settings.chain_apple_certificates();
            signing_settings
                .set_team_id_from_signing_certificate()
                .context("signing certificate is missing team id")?;
            signing_settings.set_time_stamp_url("http://timestamp.apple.com/ts01")?;
            if let Some(entitlements) = self.entitlements.as_ref() {
                let mut buf = vec![];
                entitlements.to_writer_xml(&mut buf)?;
                let entitlements = std::str::from_utf8(&buf)?;
                signing_settings.set_entitlements_xml(SettingsScope::Main, entitlements)?;
            }
            if !self.ios() {
                signing_settings
                    .set_code_signature_flags(SettingsScope::Main, CodeSignatureFlags::RUNTIME);
            }
            let bundle_signer = BundleSigner::new_from_path(self.appdir())?;
            bundle_signer.write_signed_bundle(self.appdir(), &signing_settings)?;
        }
        Ok(())
    }

    pub fn sign_dmg(&self, path: &Path, signer: &Signer) -> Result<()> {
        println!("signing {}", path.display());
        let mut f = OpenOptions::new().read(true).write(true).open(path)?;
        let mut signing_settings = SigningSettings::default();
        let cert = CapturedX509Certificate::from_der(rasn::der::encode(signer.cert()).unwrap())?;
        let secret = signer.key().to_pkcs8_der().unwrap();
        let key = InMemorySigningKeyPair::from_pkcs8_der(secret.as_bytes())?;
        signing_settings.set_signing_key(&key, cert);
        signing_settings.chain_apple_certificates();
        signing_settings
            .set_team_id_from_signing_certificate()
            .context("signing certificate is missing team id")?;
        signing_settings.set_time_stamp_url("http://timestamp.apple.com/ts01")?;
        signing_settings.set_binary_identifier(
            SettingsScope::Main,
            self.info.bundle_identifier.as_ref().unwrap(),
        );
        DmgSigner::default().sign_file(&signing_settings, &mut f)?;
        Ok(())
    }
}

pub fn app_bundle_identifier(bundle: &Path) -> Result<String> {
    let info = std::fs::read(bundle.join("Info.plist"))?;
    let info: plist::Value = plist::from_reader_xml(&*info)?;
    let bundle_identifier = info
        .as_dictionary()
        .context("invalid Info.plist")?
        .get("CFBundleIdentifier")
        .context("invalid Info.plist")?
        .as_string()
        .context("invalid Info.plist")?;
    Ok(bundle_identifier.to_string())
}

pub fn notarize(path: &Path, api_issuer: &str, api_key: &str) -> Result<()> {
    println!("notarizing {}", path.display());
    let mut notarizer = Notarizer::new()?;
    notarizer.set_api_key(api_issuer, api_key)?;
    let submission_id =
        if let NotarizationUpload::UploadId(submission_id) = notarizer.notarize_path(path, None)? {
            submission_id
        } else {
            anyhow::bail!("impossible");
        };
    println!("submission id: {}", submission_id);
    let start_time = Instant::now();
    loop {
        let resp = notarizer.wait_on_notarization(&submission_id, Duration::from_secs(3))?;
        let status = resp.data.attributes.status;
        let elapsed = start_time.elapsed();
        println!("poll state after {}s: {:?}", elapsed.as_secs(), status,);
        if status != SubmissionResponseStatus::InProgress {
            let log = notarizer.fetch_notarization_log(&submission_id)?;
            println!("{}", log);
            resp.into_result()?;
            break;
        }
    }
    let stapler = Stapler::new()?;
    stapler.staple_path(path)?;
    Ok(())
}
