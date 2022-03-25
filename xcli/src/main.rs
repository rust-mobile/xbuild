use anyhow::Result;
use appbundle::AppBundle;
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use xapk::zip::read::ZipArchive;
use xapk::Apk;
use xappimage::AppImage;
use xcli::cargo::CrateType;
use xcli::flutter::Flutter;
use xcli::maven::{FlutterEmbedding, FlutterEngine, R8};
use xcli::{command, Arch, BuildArgs, BuildEnv, CompileTarget, Format, Opt, Platform};
use xcommon::ZipFileOptions;
use xmsix::Msix;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn main() -> Result<()> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_log::LogTracer::init().ok();
    let env = std::env::var(EnvFilter::DEFAULT_ENV).unwrap_or_else(|_| "error".into());
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_span_events(FmtSpan::ACTIVE | FmtSpan::CLOSE)
        .with_env_filter(EnvFilter::new(env))
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();
    log_panics::init();
    let args = Args::parse();
    args.command.run().await
}

#[derive(Subcommand)]
enum Commands {
    Doctor,
    Devices,
    New {
        name: String,
    },
    Build {
        #[clap(flatten)]
        args: BuildArgs,
    },
    Run {
        #[clap(flatten)]
        args: BuildArgs,
        #[clap(long)]
        no_build: bool,
    },
    Lldb {
        #[clap(flatten)]
        args: BuildArgs,
    },
    Attach {
        #[clap(long)]
        url: String,
        #[clap(long)]
        root_dir: PathBuf,
        #[clap(long)]
        target_file: PathBuf,
        #[clap(long)]
        host_vmservice_port: Option<u16>,
    },
}

impl Commands {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Doctor => command::doctor(),
            Self::Devices => command::devices()?,
            Self::New { name } => command::new(&name)?,
            Self::Build { args } => {
                let env = BuildEnv::new(args)?;
                build(&env).await?;
            }
            Self::Run { args, no_build } => {
                let env = BuildEnv::new(args)?;
                if !no_build {
                    build(&env).await?;
                }
                run(&env).await?;
            }
            Self::Lldb { args } => {
                let env = BuildEnv::new(args)?;
                build(&env).await?;
                lldb(&env).await?;
            }
            Self::Attach {
                url,
                root_dir,
                target_file,
                host_vmservice_port,
            } => command::attach(&url, &root_dir, &target_file, host_vmservice_port).await?,
        }
        Ok(())
    }
}

fn download_sdk(
    build_dir: &Path,
    artifact: &str,
    no_symlinks: bool,
    no_colons: bool,
) -> Result<()> {
    xcli::download::github_release_tar_zst(
        build_dir,
        "cloudpeers",
        "x",
        "v0.1.0+2",
        artifact,
        no_symlinks,
        no_colons,
    )
}

async fn run(env: &BuildEnv) -> Result<()> {
    let out = env.executable();
    if let Some(device) = env.target().device() {
        device.run(&out, &env, env.has_dart_code()).await?;
    } else {
        anyhow::bail!("no device specified");
    }
    Ok(())
}

async fn lldb(env: &BuildEnv) -> Result<()> {
    if let Some(device) = env.target().device() {
        let target = CompileTarget::new(device.platform()?, device.arch()?, env.target().opt());
        let cargo_dir = env
            .build_dir()
            .join(target.opt().to_string())
            .join(target.platform().to_string())
            .join(target.arch().to_string())
            .join("cargo");
        let executable = env.cargo_artefact(&cargo_dir, target, CrateType::Cdylib)?;
        if let Some(lldb_server) = env.lldb_server(target) {
            device.lldb(&lldb_server, &executable)?;
        } else {
            anyhow::bail!("lldb-server not found");
        }
    } else {
        anyhow::bail!("no device specified");
    }
    Ok(())
}

async fn build(env: &BuildEnv) -> Result<()> {
    let opt_dir = env.build_dir().join(env.target().opt().to_string());
    let platform_dir = opt_dir.join(env.target().platform().to_string());
    std::fs::create_dir_all(&platform_dir)?;
    println!("package {}", env.cargo().package());
    println!("root_dir {}", env.cargo().root_dir().display());
    println!("target_dir {}", env.cargo().target_dir().display());

    match env.target().platform() {
        Platform::Linux => {
            if Platform::host()? != Platform::Linux {
                anyhow::bail!("cross compiling to linux is not yet supported");
            }
        }
        Platform::Windows => {
            if Platform::host()? != Platform::Windows {
                let windows_sdk = env.build_dir().join("Windows.sdk");
                if !windows_sdk.exists() {
                    println!("downloading windows sdk");
                    let no_symlinks = !cfg!(target_os = "linux");
                    download_sdk(env.build_dir(), "Windows.sdk.tar.zst", no_symlinks, false)?;
                }
            }
        }
        Platform::Macos => {
            if Platform::host()? != Platform::Macos {
                let macos_sdk = env.build_dir().join("MacOSX.sdk");
                if !macos_sdk.exists() {
                    println!("downloading macos sdk");
                    let no_colons = cfg!(target_os = "windows");
                    download_sdk(env.build_dir(), "MacOSX.sdk.tar.zst", false, no_colons)?;
                }
            }
        }
        Platform::Android => {
            let android_ndk = env.build_dir().join("Android.ndk");
            if !android_ndk.exists() {
                println!("downloading android ndk");
                download_sdk(env.build_dir(), "Android.ndk.tar.zst", false, false)?;
            }
        }
        Platform::Ios => {
            if Platform::host()? != Platform::Macos {
                let ios_sdk = env.build_dir().join("iPhoneOS.sdk");
                if !ios_sdk.exists() {
                    println!("downloading ios sdk");
                    let no_colons = cfg!(target_os = "windows");
                    download_sdk(env.build_dir(), "iPhoneOS.sdk.tar.zst", false, no_colons)?;
                }
            }
        }
    }

    if let Some(flutter) = env.flutter() {
        flutter.upgrade()?;
        let engine_version = flutter.engine_version()?;
        let host = CompileTarget::new(Platform::host()?, Arch::host()?, Opt::Debug);
        let mut engine_version_changed = false;
        for target in env.target().compile_targets().chain(std::iter::once(host)) {
            let engine_dir = flutter.engine_dir(target)?;
            if !engine_dir.exists() {
                println!("downloading flutter engine for {}", target);
                xcli::download::flutter_engine(&engine_dir, &engine_version, target)?;
                engine_version_changed = true;
            }
        }
        if !env
            .root_dir()
            .join(".dart_tool")
            .join("package_config.json")
            .exists()
            || xcommon::stamp_file(env.pubspec(), &env.build_dir().join("pubspec.stamp"))?
        {
            println!("pub get");
            flutter.pub_get(env.root_dir())?;
        }
        if env.target().platform() == Platform::Android {
            if engine_version_changed || !platform_dir.join("classes.dex").exists() {
                println!("building classes.dex");
                build_classes_dex(&env, &flutter, &platform_dir, env.target().opt())?;
            }
        }
        println!("building flutter_assets");
        flutter.build_flutter_assets(
            env.root_dir(),
            &env.build_dir().join("flutter_assets"),
            &env.build_dir().join("flutter_assets.d"),
        )?;
        println!("building kernel_blob.bin");
        let kernel_blob = platform_dir.join("kernel_blob.bin");
        flutter.kernel_blob_bin(
            env.root_dir(),
            env.target_file(),
            &kernel_blob,
            &platform_dir.join("kernel_blob.bin.d"),
            env.target().opt(),
        )?;
        if env.target().opt() == Opt::Release
            && xcommon::stamp_file(&kernel_blob, &platform_dir.join("kernel_blob.bin.stamp"))?
        {
            for target in env.target().compile_targets() {
                println!("building aot snapshot for {}", target);
                let arch_dir = platform_dir.join(target.arch().to_string());
                std::fs::create_dir_all(&arch_dir)?;
                flutter.aot_snapshot(
                    env.root_dir(),
                    &kernel_blob,
                    &arch_dir.join("libapp.so"),
                    target,
                )?;
            }
        }
    }

    let bin_target = env.target().platform() != Platform::Android
        && (env.flutter().is_some() || env.target().platform() != Platform::Ios);
    let has_lib = env.root_dir().join("src").join("lib.rs").exists();
    if bin_target || has_lib {
        for target in env.target().compile_targets() {
            let arch_dir = platform_dir.join(target.arch().to_string());
            let mut cargo = env.cargo_build(target, &arch_dir.join("cargo"))?;
            let artifact = if bin_target {
                "binary"
            } else {
                cargo.arg("--lib");
                "library"
            };
            println!("building rust {} for {}", artifact, target);
            cargo.exec()?;
        }
    }

    println!("building {}", env.target().format());
    let out = match env.target().platform() {
        Platform::Linux => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());

            let appimage = AppImage::new(&arch_dir, env.name().to_string())?;
            appimage.add_apprun()?;
            appimage.add_desktop()?;
            if let Some(icon) = env.icon() {
                appimage.add_icon(icon)?;
            }

            if let Some(flutter) = env.flutter() {
                let engine_dir = flutter.engine_dir(target)?;
                appimage.add_file(
                    &flutter.icudtl_dat()?,
                    &Path::new("data").join("icudtl.dat"),
                )?;
                appimage.add_file(
                    &engine_dir.join("libflutter_linux_gtk.so"),
                    &Path::new("lib").join("libflutter_linux_gtk.so"),
                )?;
                appimage.add_directory(
                    &env.build_dir().join("flutter_assets"),
                    &Path::new("data").join("flutter_assets"),
                )?;
                match target.opt() {
                    Opt::Debug => {
                        appimage.add_file(
                            &platform_dir.join("kernel_blob.bin"),
                            &Path::new("data")
                                .join("flutter_assets")
                                .join("kernel_blob.bin"),
                        )?;
                    }
                    Opt::Release => {
                        appimage.add_file(
                            &arch_dir.join("libapp.so"),
                            &Path::new("lib").join("libapp.so"),
                        )?;
                    }
                }
            }

            let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
            appimage.add_file(&main, Path::new(env.name()))?;

            if env.target().format() == Format::Appimage {
                let out = arch_dir.join(format!("{}.AppImage", env.name()));
                appimage.build(&out, env.target().signer().cloned())?;
                out
            } else {
                appimage.appdir().join("AppRun")
            }
        }
        Platform::Android => {
            let out = platform_dir.join(format!("{}.apk", env.name()));
            let mut apk = Apk::new(out.clone(), env.manifest().android().clone())?;
            apk.add_res(env.icon(), &env.android_jar()?)?;
            if let Some(flutter) = env.flutter() {
                // add libflutter.so
                let engine_version = flutter.engine_version()?;
                for target in env.target().compile_targets() {
                    let abi = target.android_abi();
                    let flutter_engine = FlutterEngine::new(abi, target.opt(), &engine_version);
                    let flutter_jar = env
                        .maven()?
                        .package(&flutter_engine.package(), &flutter_engine.version())?;
                    let mut zip = ZipArchive::new(BufReader::new(File::open(flutter_jar)?))?;
                    let f = zip.by_name(&format!("lib/{}/libflutter.so", abi.android_abi()))?;
                    apk.add_zip_file(f)?;
                }
                apk.add_dex(&platform_dir.join("classes.dex"))?;
                apk.add_directory(
                    &env.build_dir().join("flutter_assets"),
                    &Path::new("assets").join("flutter_assets"),
                )?;
                apk.add_file(
                    &flutter.vm_snapshot_data()?,
                    &Path::new("assets")
                        .join("flutter_assets")
                        .join("vm_snapshot_data"),
                    ZipFileOptions::Compressed,
                )?;
                apk.add_file(
                    &flutter.isolate_snapshot_data()?,
                    &Path::new("assets")
                        .join("flutter_assets")
                        .join("isolate_snapshot_data"),
                    ZipFileOptions::Compressed,
                )?;
                match env.target().opt() {
                    Opt::Debug => {
                        apk.add_file(
                            &platform_dir.join("kernel_blob.bin"),
                            &Path::new("assets")
                                .join("flutter_assets")
                                .join("kernel_blob.bin"),
                            ZipFileOptions::Compressed,
                        )?;
                    }
                    Opt::Release => {
                        for target in env.target().compile_targets() {
                            apk.add_lib(
                                target.android_abi(),
                                &platform_dir
                                    .join(target.arch().to_string())
                                    .join("libapp.so"),
                            )?;
                        }
                    }
                }
            }
            if has_lib {
                for target in env.target().compile_targets() {
                    let arch_dir = platform_dir.join(target.arch().to_string());
                    let lib =
                        env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Cdylib)?;
                    apk.add_lib(target.android_abi(), &lib)?;
                }
            }
            apk.finish(env.target().signer().cloned())?;
            out
        }
        Platform::Macos => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());
            std::fs::create_dir_all(&arch_dir)?;
            let mut app = AppBundle::new(&arch_dir, env.manifest().macos().clone())?;
            if let Some(icon) = env.icon() {
                app.add_icon(icon)?;
            }
            if let Some(flutter) = env.flutter() {
                app.add_framework(&flutter.engine_dir(target)?.join("FlutterMacOS.framework"))?;
                app.add_directory(
                    &env.build_dir().join("flutter_assets"),
                    Path::new("flutter_assets"),
                )?;
                match target.opt() {
                    Opt::Debug => {
                        app.add_file(
                            &platform_dir.join("kernel_blob.bin"),
                            &Path::new("flutter_assets").join("kernel_blob.bin"),
                        )?;
                    }
                    Opt::Release => {
                        app.add_file(
                            &arch_dir.join("libapp.so"),
                            &Path::new("flutter_assets").join("libapp.so"),
                        )?;
                    }
                }
            }
            let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
            app.add_executable(&main)?;
            let appdir = app.finish(env.target().signer().cloned())?;
            if env.target().format() == Format::Dmg {
                let out = arch_dir.join(format!("{}.dmg", env.name()));
                appbundle::make_dmg(&arch_dir, &appdir, &out)?;
                out
            } else {
                appdir
            }
        }
        Platform::Ios => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());
            std::fs::create_dir_all(&arch_dir)?;
            let mut app = AppBundle::new(&arch_dir, env.manifest().ios().clone())?;
            // TODO:
            /*if let Some(icon) = env.icon() {
                app.add_icon(icon)?;
            }*/
            if let Some(flutter) = env.flutter() {
                let framework = flutter
                    .engine_dir(target)?
                    .join("Flutter.xcframework")
                    .join("ios-arm64_armv7")
                    .join("Flutter.framework");
                app.add_framework(&framework)?;
                app.add_directory(
                    &env.build_dir().join("flutter_assets"),
                    Path::new("flutter_assets"),
                )?;
                match target.opt() {
                    Opt::Debug => {
                        app.add_file(
                            &platform_dir.join("kernel_blob.bin"),
                            &Path::new("flutter_assets").join("kernel_blob.bin"),
                        )?;
                    }
                    Opt::Release => {
                        app.add_file(
                            &arch_dir.join("libapp.so"),
                            &Path::new("flutter_assets").join("libapp.so"),
                        )?;
                    }
                }
                build_ios_main(&env, flutter, &arch_dir, target)?;
                app.add_executable(&arch_dir.join("main"))?;
            } else {
                let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
                app.add_executable(&main)?;
            }
            app.add_provisioning_profile(env.target().provisioning_profile().unwrap())?;
            app.finish(env.target().signer().cloned())?
        }
        Platform::Windows => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());
            std::fs::create_dir_all(&arch_dir)?;
            let out = arch_dir.join(format!("{}.msix", env.name()));
            let mut msix = Msix::new(out.clone(), env.manifest().windows().clone())?;
            if let Some(icon) = env.icon() {
                msix.add_icon(icon)?;
            }
            // TODO: *.pri

            if let Some(flutter) = env.flutter() {
                let engine_dir = flutter.engine_dir(target)?;
                msix.add_file(
                    &flutter.icudtl_dat()?,
                    &Path::new("data").join("icudtl.dat"),
                    ZipFileOptions::Compressed,
                )?;
                msix.add_file(
                    &engine_dir.join("flutter_windows.dll"),
                    &Path::new("flutter_windows.dll"),
                    ZipFileOptions::Compressed,
                )?;
                msix.add_directory(
                    &env.build_dir().join("flutter_assets"),
                    &Path::new("data").join("flutter_assets"),
                )?;
                match target.opt() {
                    Opt::Debug => {
                        msix.add_file(
                            &platform_dir.join("kernel_blob.bin"),
                            &Path::new("data")
                                .join("flutter_assets")
                                .join("kernel_blob.bin"),
                            ZipFileOptions::Compressed,
                        )?;
                    }
                    Opt::Release => {
                        msix.add_file(
                            &arch_dir.join("libapp.so"),
                            &Path::new("data").join("app.so"),
                            ZipFileOptions::Compressed,
                        )?;
                    }
                }
            }
            let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
            msix.add_file(
                &main,
                format!("{}.exe", env.name()).as_ref(),
                ZipFileOptions::Compressed,
            )?;
            msix.finish(env.target().signer().cloned())?;
            out
        }
    };
    println!("built {}", out.display());
    Ok(())
}

fn build_classes_dex(
    env: &BuildEnv,
    flutter: &Flutter,
    platform_dir: &Path,
    opt: Opt,
) -> Result<()> {
    let engine_version = flutter.engine_version()?;
    let android_jar = env.android_jar()?;
    let flutter_embedding = FlutterEmbedding::new(env.target().opt(), &engine_version);
    let deps = env
        .maven()?
        .resolve(flutter_embedding.package(), flutter_embedding.version())?
        .into_iter()
        .filter(|path| {
            path.extension() == Some("jar".as_ref()) || path.extension() == Some("aar".as_ref())
        })
        .collect::<Vec<_>>();
    let r8 = R8::new(3, 1, 51);
    let r8 = env.maven()?.package(&r8.package(), &r8.version())?;

    // build GeneratedPluginRegistrant
    let plugins = platform_dir.join("GeneratedPluginRegistrant.java");
    std::fs::write(
        &plugins,
        include_bytes!("../assets/GeneratedPluginRegistrant.java"),
    )?;
    let separator = if cfg!(windows) { ";" } else { ":" };
    let classpath = deps
        .iter()
        .chain(std::iter::once(&android_jar))
        .map(|d| d.display().to_string())
        .collect::<Vec<_>>()
        .join(separator);
    let java = platform_dir.join("java");
    let status = Command::new("javac")
        .arg("--class-path")
        .arg(classpath)
        .arg(plugins)
        .arg("-d")
        .arg(&java)
        .status()?;
    if !status.success() {
        anyhow::bail!("javac exited with nonzero exit code.");
    }

    // build classes.dex
    let pg = platform_dir.join("proguard-rules.pro");
    std::fs::write(&pg, include_bytes!("../assets/proguard-rules.pro"))?;
    let plugins = java
        .join("io")
        .join("flutter")
        .join("plugins")
        .join("GeneratedPluginRegistrant.class");
    let mut java = Command::new("java");
    java.arg("-cp")
        .arg(r8)
        .arg("com.android.tools.r8.R8")
        .args(deps)
        .arg(plugins)
        .arg("--lib")
        .arg(android_jar)
        .arg("--output")
        .arg(platform_dir)
        .arg("--pg-conf")
        .arg(pg);
    if opt == Opt::Release {
        java.arg("--release");
    }
    if !java.status()?.success() {
        anyhow::bail!("`{:?}` exited with nonzero exit code.", java);
    }
    Ok(())
}

fn build_ios_main(
    env: &BuildEnv,
    flutter: &Flutter,
    arch_dir: &Path,
    target: CompileTarget,
) -> Result<()> {
    let sdk = env.build_dir().join("iPhoneOS.sdk");
    let main_m = arch_dir.join("main.m");
    let main = arch_dir.join("main");
    let framework = flutter
        .engine_dir(target)?
        .join("Flutter.xcframework")
        .join("ios-arm64_armv7");
    std::fs::write(&main_m, include_bytes!("../assets/main.m"))?;
    let status = Command::new("clang")
        .arg("-objc")
        .arg("-fmodules")
        .arg("--target=arm64-apple-ios")
        .arg(format!("--sysroot={}", sdk.display()))
        .arg("-F")
        .arg(framework)
        .arg("-fuse-ld=lld")
        .arg("-o")
        .arg(&main)
        .arg(&main_m)
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to build main.m");
    }
    Ok(())
}
