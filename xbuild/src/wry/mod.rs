use crate::cargo::CrateType;
use crate::{task, BuildEnv};
use anyhow::Result;
use std::path::Path;
use std::process::Command;

static BUILD_GRADLE: &[u8] = include_bytes!("./build.gradle");
static GRADLE_PROPERTIES: &[u8] = include_bytes!("./gradle.properties");
static SETTINGS_GRADLE: &[u8] = include_bytes!("./settings.gradle");

pub fn setup_env(env: &BuildEnv) -> Result<()> {
    let gradle = env.platform_dir().join("gradle");
    let app = gradle.join("app");
    let kotlin = app.join("src").join("main").join("kotlin");

    std::fs::create_dir_all(&kotlin)?;
    std::fs::write(gradle.join("build.gradle"), BUILD_GRADLE)?;
    std::fs::write(gradle.join("gradle.properties"), GRADLE_PROPERTIES)?;
    std::fs::write(gradle.join("settings.gradle"), SETTINGS_GRADLE)?;

    let manifest = env.manifest().android();
    let package = manifest.package.clone().unwrap_or_default();
    let target_sdk = manifest.sdk.target_sdk_version.unwrap();
    let min_sdk = manifest.sdk.min_sdk_version.unwrap();
    let version_code = manifest.version_code.unwrap();
    let version_name = manifest.version_name.as_ref().unwrap();

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
            }}
            dependencies {{
                implementation 'androidx.appcompat:appcompat:1.4.1'
            }}
        "#,
        package = package,
        target_sdk = target_sdk,
        min_sdk = min_sdk,
        version_code = version_code,
        version_name = version_name,
    );
    std::fs::write(app.join("build.gradle"), app_build_gradle)?;

    let main_activity = format!(
        r#"
            package {}
            class MainActivity : TauriActivity()
        "#,
        package,
    );
    std::fs::write(kotlin.join("MainActivity.kt"), main_activity)?;

    let (package, name) = package.rsplit_once('.').unwrap();
    std::env::set_var("WRY_ANDROID_REVERSED_DOMAIN", package);
    std::env::set_var("WRY_ANDROID_APP_NAME_SNAKE_CASE", name);
    std::env::set_var("WRY_ANDROID_KOTLIN_FILES_OUT_DIR", kotlin);
    Ok(())
}

pub fn build_apk(env: &BuildEnv, apk: &Path) -> Result<()> {
    let platform_dir = env.platform_dir();
    let gradle = platform_dir.join("gradle");
    let main = gradle.join("app").join("src").join("main");
    let jnilibs = main.join("jniLibs");

    let mut manifest = env.manifest().android().clone();
    manifest.package = None;
    manifest.version_code = None;
    manifest.version_name = None;
    manifest.compile_sdk_version = None;
    manifest.compile_sdk_version_codename = None;
    manifest.platform_build_version_code = None;
    manifest.platform_build_version_name = None;
    manifest.sdk = Default::default();
    manifest.application.debuggable = None;
    std::fs::write(
        main.join("AndroidManifest.xml"),
        quick_xml::se::to_string(&manifest)?,
    )?;

    for target in env.target().compile_targets() {
        let arch_dir = platform_dir.join(target.arch().to_string());
        let lib = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Cdylib)?;
        let lib_name = lib.file_name().unwrap();
        let lib_dir = jnilibs.join(target.android_abi().android_abi());
        std::fs::create_dir_all(&lib_dir)?;
        std::fs::copy(&lib, lib_dir.join(lib_name))?;
    }

    let mut cmd = Command::new("gradle");
    cmd.current_dir(&gradle).arg("build");
    task::run(cmd, env.verbose())?;
    let out = gradle
        .join("app")
        .join("build")
        .join("outputs")
        .join("apk")
        .join("debug")
        .join("app-debug.apk");
    std::fs::copy(out, apk)?;
    Ok(())
}
