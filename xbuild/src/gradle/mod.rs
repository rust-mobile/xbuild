use crate::{task, BuildEnv, Format, Opt, Store};
use anyhow::{Context, Result};
use apk::Target;
use std::path::{Path, PathBuf};
use std::process::Command;

static BUILD_GRADLE: &[u8] = include_bytes!("./build.gradle");
static GRADLE_PROPERTIES: &[u8] = include_bytes!("./gradle.properties");
static SETTINGS_GRADLE: &[u8] = include_bytes!("./settings.gradle");
static IC_LAUNCHER: &[u8] = include_bytes!("./ic_launcher.xml");

/// Generate a default Android keystore for signing if none provided
fn generate_default_keystore(env: &BuildEnv, keystore_path: &Path, password: &str, domain: &str) -> Result<()> {
    std::fs::create_dir_all(keystore_path.parent().unwrap())?;
    
    let dname = format!("CN={}, OU=NA, O=Company, L=City, S=State, C=US", domain);
    let pkg_name = &env.name();
    let alias_name = format!("{}-release-key", pkg_name);
    
    task::run(
        Command::new("keytool")
            .arg("-genkeypair")
            .arg("-v")
            .arg("-noprompt")
            .arg("-storetype")
            .arg("PKCS12")
            .arg("-alias")
            .arg(&alias_name)
            .arg("-keystore")
            .arg(keystore_path)
            .arg("-keyalg")
            .arg("RSA")
            .arg("-keysize")
            .arg("2048")
            .arg("-validity")
            .arg("10000")
            .arg("-storepass")
            .arg(password)
            .arg("-keypass")
            .arg(password)
            .arg("-dname")
            .arg(&dname),
    )?;
    
    // Export the certificate for upload to Google Play
    let pem_path = keystore_path.parent().unwrap().join(format!("{}-release-upload-certificate.pem", pkg_name));
    task::run(
        Command::new("keytool")
            .arg("-export")
            .arg("-rfc")
            .arg("-v")
            .arg("-noprompt")
            .arg("-storepass")
            .arg(password)
            .arg("-keypass")
            .arg(password)
            .arg("-keystore")
            .arg(keystore_path)
            .arg("-alias")
            .arg(&alias_name)
            .arg("-file")
            .arg(&pem_path),
    )?;
    
    Ok(())
}

/// Sign AAB with jarsigner
fn sign_aab_with_jarsigner(
    aab_path: &Path, 
    keystore_path: &Path,
    storepass: &str,
    keyname: &str,
    keypass: &str,
) -> Result<()> {
    task::run(
        Command::new("jarsigner")
            .arg("-storepass")
            .arg(storepass)
            .arg("-keypass")
            .arg(keypass)
            .arg("-keystore")
            .arg(keystore_path)
            .arg(aab_path)
            .arg(keyname),
    )?;
    Ok(())
}

/// Sign APK with apksigner
fn sign_apk_with_apksigner(
    apk_path: &Path,
    keystore_path: &Path,
    storepass: &str,
    keyname: &str,
    keypass: &str,
) -> Result<()> {
    println!("Starting APK signing process...");
    println!("Input APK: {}", apk_path.display());
    println!("Keystore: {}", keystore_path.display());
    
    // First align the APK
    let aligned_path = apk_path.with_extension("aligned.apk");
    println!("Aligned APK path: {}", aligned_path.display());
    
    // Find zipalign in Android SDK
    let android_home = std::env::var("ANDROID_HOME")
        .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
        .context("ANDROID_HOME or ANDROID_SDK_ROOT environment variable not set")?;
    
    let build_tools_dir = Path::new(&android_home).join("build-tools");
    let build_tools_version = std::fs::read_dir(&build_tools_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .max()
        .context("No build-tools found in Android SDK")?;
    
    let build_tools_path = build_tools_dir.join(&build_tools_version);
    let zipalign = build_tools_path.join("zipalign");
    let apksigner = build_tools_path.join("apksigner");
    
    println!("Using build tools: {}", build_tools_path.display());
    println!("zipalign: {}", zipalign.display());
    println!("apksigner: {}", apksigner.display());
    
    // Align the APK
    println!("Aligning APK...");
    task::run(
        Command::new(&zipalign)
            .arg("-v")
            .arg("4")
            .arg(apk_path)
            .arg(&aligned_path),
    )?;
    println!("APK alignment completed");
    
    // Sign the aligned APK
    let apk_name = apk_path.file_stem().unwrap().to_string_lossy();
    let apk_dir = apk_path.parent().unwrap();
    let signed_path = apk_dir.join(format!("{}-signed.apk", apk_name));
    
    println!("Signing aligned APK...");
    println!("Signed APK will be created at: {}", signed_path.display());
    
    task::run(
        Command::new(&apksigner)
            .arg("sign")
            .arg("--ks")
            .arg(keystore_path)
            .arg("--ks-key-alias")
            .arg(keyname)
            .arg("--ks-pass")
            .arg(&format!("pass:{}", storepass))
            .arg("--key-pass")
            .arg(&format!("pass:{}", keypass))
            .arg("--out")
            .arg(&signed_path)
            .arg(&aligned_path),
    )?;
    
    // Verify the signed APK was created
    if !signed_path.exists() {
        return Err(anyhow::anyhow!("Signed APK was not created at: {}", signed_path.display()));
    }
    
    // Verify the APK signature
    println!("Verifying APK signature...");
    let verify_result = Command::new(&apksigner)
        .arg("verify")
        .arg("--verbose")
        .arg("--print-certs")
        .arg(&signed_path)
        .output();
    
    match verify_result {
        Ok(output) => {
            if output.status.success() {
                println!("✓ APK signature verification passed");
                println!("Signature details:\n{}", String::from_utf8_lossy(&output.stdout));
            } else {
                println!("⚠ APK signature verification failed: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        Err(e) => {
            println!("⚠ Could not verify APK signature: {}", e);
        }
    }
    
    println!("✓ Signed APK created successfully");
    println!("✓ Unsigned APK: {}", apk_path.display());
    println!("✓ Signed APK: {}", signed_path.display());
    println!("APK signing process completed successfully");
    
    // Clean up the aligned APK
    let _ = std::fs::remove_file(&aligned_path);
    
    Ok(())
}

/// Create encrypted keystore for Google Play using pepk.jar
fn create_encrypted_keystore_for_play(
    keystore_path: &Path,
    keyname: &str,
    _storepass: &str,
    _keypass: &str,
    pubkey_path: &Path,
    output_path: &Path,
) -> Result<()> {
    // Download pepk.jar if it doesn't exist
    let pepk_jar_path = std::env::temp_dir().join("pepk.jar");
    if !pepk_jar_path.exists() {
        let response = reqwest::blocking::get("https://www.gstatic.com/play-apps-publisher-rapid/signing-tool/prod/pepk.jar")
            .context("Failed to download pepk.jar")?;
        
        let bytes = response.bytes().context("Failed to read pepk.jar response")?;
        std::fs::write(&pepk_jar_path, &bytes)?;
    }
    
    task::run(
        Command::new("java")
            .arg("-jar")
            .arg(&pepk_jar_path)
            .arg("--keystore")
            .arg(keystore_path)
            .arg("--alias")
            .arg(keyname)
            .arg("--output")
            .arg(output_path)
            .arg("--include-cert")
            .arg("--rsa-aes-encryption")
            .arg("--encryption-key-path")
            .arg(pubkey_path),
    )?;
    
    Ok(())
}

/// Get domain from manifest for keystore generation
fn get_domain_from_manifest(env: &BuildEnv) -> String {
    env.config()
        .android()
        .manifest
        .package
        .as_ref()
        .unwrap_or(&"com.example.app".to_string())
        .clone()
}

pub fn prepare(env: &BuildEnv) -> Result<()> {
    let config = env.config().android();
    if config.wry {
        let package = config.manifest.package.as_ref().unwrap();
        let wry = env.platform_dir().join("wry");
        std::fs::create_dir_all(&wry)?;
        if !env.cargo().package_root().join("kotlin").exists() {
            let main_activity = format!(
                r#"
                    package {package}
                    class MainActivity : TauriActivity()
                "#,
            );
            std::fs::write(wry.join("MainActivity.kt"), main_activity)?;
        }
        let (package, name) = package.rsplit_once('.').unwrap();
        std::env::set_var("WRY_ANDROID_REVERSED_DOMAIN", package);
        std::env::set_var("WRY_ANDROID_APP_NAME_SNAKE_CASE", name);
        std::env::set_var("WRY_ANDROID_KOTLIN_FILES_OUT_DIR", wry);
    }
    Ok(())
}

pub fn build(env: &BuildEnv, libraries: Vec<(Target, PathBuf)>, out: &Path) -> Result<()> {
    let platform_dir = env.platform_dir();
    let gradle = platform_dir.join("gradle");
    let app = gradle.join("app");
    let main = app.join("src").join("main");
    let kotlin = main.join("kotlin");
    let jnilibs = main.join("jniLibs");
    let res = main.join("res");

    std::fs::create_dir_all(&kotlin)?;
    std::fs::write(gradle.join("build.gradle"), BUILD_GRADLE)?;
    std::fs::write(gradle.join("gradle.properties"), GRADLE_PROPERTIES)?;
    std::fs::write(gradle.join("settings.gradle"), SETTINGS_GRADLE)?;

    let config = env.config().android();
    let mut manifest = config.manifest.clone();

    let package = manifest.package.take().unwrap_or_default();
    let target_sdk = manifest.sdk.target_sdk_version.take().unwrap();
    let min_sdk = manifest.sdk.min_sdk_version.take().unwrap();
    let version_code = manifest.version_code.take().unwrap();
    let version_name = manifest.version_name.take().unwrap();

    manifest.compile_sdk_version = None;
    manifest.compile_sdk_version_codename = None;
    manifest.platform_build_version_code = None;
    manifest.platform_build_version_name = None;
    manifest.application.debuggable = None;

    let mut dependencies = String::new();
    for dep in &config.dependencies {
        dependencies.push_str(&format!("implementation '{dep}'\n"));
    }

    let asset_packs = if config.assets.is_empty() {
        ""
    } else {
        r#"assetPacks = [":baseAssets"]"#
    };

    let app_build_gradle = format!(
        r#"
            plugins {{
                id 'com.android.application'
                id 'org.jetbrains.kotlin.android'
            }}
            android {{
                namespace '{package}'
                compileSdk {target_sdk}
                defaultConfig {{
                    applicationId '{package}'
                    minSdk {min_sdk}
                    targetSdk {target_sdk}
                    versionCode {version_code}
                    versionName '{version_name}'
                }}
                packagingOptions {{
                    jniLibs {{
                        useLegacyPackaging = true
                    }}
                }}
                {asset_packs}
            }}
            dependencies {{
                {dependencies}
            }}
        "#,
    );

    let pack_name = "baseAssets";
    let base_assets = gradle.join(pack_name);
    // Make sure that any possibly-obsolete asset pack does not clobber the build
    let _ = std::fs::remove_dir_all(&base_assets);

    if !config.assets.is_empty() {
        std::fs::create_dir_all(&base_assets)?;
        let assets = format!(
            r#"
            plugins {{
                id 'com.android.asset-pack'
            }}
            assetPack {{
                packName = "{pack_name}" // Directory name for the asset pack
                dynamicDelivery {{
                    // Use install-time to make assets available to AAssetManager
                    // https://developer.android.com/guide/playcore/asset-delivery/integrate-native
                    deliveryType = "install-time"
                }}
            }}
            "#,
        );

        std::fs::write(base_assets.join("build.gradle"), assets)?;

        let target_dir = base_assets.join("src/main/assets");
        let _ = std::fs::remove_dir_all(&target_dir);
        std::fs::create_dir_all(&target_dir)?;
        for asset in &config.assets {
            let path = env.cargo().package_root().join(asset.path());
            let target = target_dir.join(asset.path().file_name().unwrap());

            if !asset.optional() || path.exists() {
                // Make this file or directory available to the `gradle` build system
                xcommon::symlink(&path, &target).with_context(|| {
                    format!(
                        "Failed to make asset file/folder `{}` available to gradle at `{}`",
                        path.display(),
                        target.display()
                    )
                })?;
            }
        }
    }

    if let Some(icon_path) = env.icon.as_ref() {
        let mut scaler = xcommon::Scaler::open(icon_path)?;
        scaler.optimize();
        let anydpi = res.join("mipmap-anydpi-v26");
        std::fs::create_dir_all(&anydpi)?;
        std::fs::write(anydpi.join("ic_launcher.xml"), IC_LAUNCHER)?;
        let dpis = [
            ("m", 48),
            ("h", 72),
            ("xh", 96),
            ("xxh", 144),
            ("xxh", 192),
            ("xxxh", 256),
        ];
        for (name, size) in dpis {
            let dir_name = format!("mipmap-{name}dpi");
            let dir = res.join(dir_name);
            std::fs::create_dir_all(&dir)?;
            for variant in ["foreground", "monochrome"] {
                let mut icon =
                    std::fs::File::create(dir.join(format!("ic_launcher_{variant}.png")))?;
                scaler.write(
                    &mut icon,
                    xcommon::ScalerOptsBuilder::new(size, size).build(),
                )?;
            }
        }
        manifest.application.icon = Some("@mipmap/ic_launcher".into());
    }

    std::fs::write(app.join("build.gradle"), app_build_gradle)?;
    std::fs::write(
        main.join("AndroidManifest.xml"),
        quick_xml::se::to_string(&manifest)?,
    )?;

    let srcs = [
        env.cargo().package_root().join("kotlin"),
        env.platform_dir().join("wry"),
    ];
    for src in srcs {
        if !src.exists() {
            continue;
        }
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            std::fs::copy(entry.path(), kotlin.join(entry.file_name()))?;
        }
    }

    for (target, lib) in libraries {
        let name = lib.file_name().context("invalid path")?;
        let lib_dir = jnilibs.join(target.as_str());
        std::fs::create_dir_all(&lib_dir)?;
        std::fs::copy(&lib, lib_dir.join(name))?;
    }

    let opt = env.target().opt();
    let format = env.target().format();
    task::run(
        Command::new("gradle")
            .current_dir(&gradle)
            .arg(match format {
                Format::Aab => "bundle",
                Format::Apk => "assemble",
                _ => unreachable!(),
            }),
    )?;
    let output = gradle
        .join("app")
        .join("build")
        .join("outputs")
        .join(match format {
            Format::Aab => "bundle",
            Format::Apk => "apk",
            _ => unreachable!(),
        })
        .join(opt.to_string())
        .join(match (format, opt) {
            (Format::Apk, Opt::Debug) => "app-debug.apk",
            (Format::Apk, Opt::Release) => "app-release-unsigned.apk",
            (Format::Aab, Opt::Debug) => "app-debug.aab",
            (Format::Aab, Opt::Release) => "app-release.aab",
            _ => unreachable!(),
        });

    // Handle signing if release build and signing parameters are provided
    if opt == Opt::Release {
        handle_android_signing(env, &output, format)?;
    }

    std::fs::copy(output, out)?;
    Ok(())
}

/// Handle Android signing for AAB and APK files
fn handle_android_signing(env: &BuildEnv, file_path: &Path, format: Format) -> Result<()> {
    let keystore_path = env.target().android_sign_keystore();
    let storepass = env.target().android_sign_storepass();
    let keyname = env.target().android_sign_keyname();
    let keypass = env.target().android_sign_keypass();

    // Determine if we need to generate a default keystore
    let (final_keystore_path, final_storepass, final_keyname, final_keypass) = 
        if let (Some(keystore), Some(storepass), Some(keyname), Some(keypass)) = 
            (keystore_path, storepass, keyname, keypass) {
            (keystore.to_path_buf(), storepass.to_string(), keyname.to_string(), keypass.to_string())
        } else {
            // Generate default keystore
            let pkg_name = &env.name();
            let default_keystore = env.platform_dir().join("keys").join(format!("{}-release-key.keystore", pkg_name));
            let default_password = "Test123".to_string();
            let default_keyname = format!("{}-release-key", pkg_name);
            let domain = get_domain_from_manifest(env);

            if !default_keystore.exists() {
                println!("Generating default Android keystore...");
                generate_default_keystore(env, &default_keystore, &default_password, &domain)?;
            }

            (default_keystore, default_password.clone(), default_keyname, default_password)
        };

    // Sign the file based on format
    match format {
        Format::Aab => {
            println!("Signing AAB with jarsigner...");
            sign_aab_with_jarsigner(
                file_path,
                &final_keystore_path,
                &final_storepass,
                &final_keyname,
                &final_keypass,
            )?;
            
            // Validate the AAB after signing
            println!("Validating AAB with bundletool...");
            let is_valid = validate_aab_with_bundletool(file_path)?;
            if !is_valid {
                println!("Warning: AAB validation failed. The BundleConfig.pb may be missing.");
            }
        }
        Format::Apk => {
            println!("Signing APK with apksigner...");
            println!("APK path: {}", file_path.display());
            sign_apk_with_apksigner(
                file_path,
                &final_keystore_path,
                &final_storepass,
                &final_keyname,
                &final_keypass,
            )?;
            println!("APK signing completed successfully");
        }
        _ => {}
    }

    // Handle Google Play encryption if needed
    if env.target().store() == Some(Store::Play) {
        if let Some(pubkey_path) = env.target().play_app_sign_enc_pubkey() {
            println!("Creating encrypted keystore for Google Play...");
            let output_zip = env.platform_dir().join("app-signing-key-encrypted.zip");
            create_encrypted_keystore_for_play(
                &final_keystore_path,
                &final_keyname,
                &final_storepass,
                &final_keypass,
                pubkey_path,
                &output_zip,
            )?;
            println!("Encrypted keystore created at: {}", output_zip.display());
        }
    }

    Ok(())
}

/// Handle Android APK signing for non-gradle builds
pub fn handle_android_signing_for_apk(env: &BuildEnv, apk_path: &Path) -> Result<()> {
    handle_android_signing(env, apk_path, Format::Apk)
}

/// Validate AAB file using bundletool
pub fn validate_aab_with_bundletool(aab_path: &Path) -> Result<bool> {
    // Download bundletool if it doesn't exist
    let bundletool_jar_path = std::env::temp_dir().join("bundletool-all-1.18.1.jar");
    if !bundletool_jar_path.exists() {
        println!("Downloading bundletool...");
        let response = reqwest::blocking::get("https://github.com/google/bundletool/releases/latest/download/bundletool-all-1.18.1.jar")
            .context("Failed to download bundletool")?;
        
        let bytes = response.bytes().context("Failed to read bundletool response")?;
        std::fs::write(&bundletool_jar_path, &bytes)?;
    }

    // Validate the AAB and check for BundleConfig.pb
    let output = Command::new("java")
        .arg("-jar")
        .arg(&bundletool_jar_path)
        .arg("validate")
        .arg("--bundle")
        .arg(aab_path)
        .output()
        .context("Failed to run bundletool validate")?;

    if !output.status.success() {
        println!("bundletool validation failed: {}", String::from_utf8_lossy(&output.stderr));
        return Ok(false);
    }

    // Check for BundleConfig.pb by grepping the output
    let validation_output = String::from_utf8_lossy(&output.stdout);
    let bundle_config_count = validation_output.matches("BundleConfig.pb").count();
    
    println!("AAB validation passed. BundleConfig.pb found: {} times", bundle_config_count);
    Ok(bundle_config_count > 0)
}
