use crate::cargo::CrateType;
use crate::download::DownloadManager;
use crate::task::TaskRunner;
use crate::{BuildEnv, Format, Opt, Platform};
use anyhow::{Context, Result};
use apk::Apk;
use appbundle::AppBundle;
use appimage::AppImage;
use msix::Msix;
use std::path::Path;
use xcommon::{Zip, ZipFileOptions};

pub fn build(env: &BuildEnv) -> Result<()> {
    let platform_dir = env.platform_dir();
    std::fs::create_dir_all(&platform_dir)?;

    let mut runner = TaskRunner::new(3, env.verbose());

    runner.start_task("Fetch precompiled artifacts");
    let manager = DownloadManager::new(env)?;
    if !env.offline() {
        manager.prefetch()?;
        runner.end_verbose_task();
    }

    runner.start_task(format!("Build rust `{}`", env.name));
    let bin_target = env.target().platform() != Platform::Android;
    let has_lib = env.root_dir().join("src").join("lib.rs").exists();
    if bin_target || has_lib {
        if env.target().platform() == Platform::Android && env.config().android().gradle {
            crate::gradle::prepare(env)?;
        }
        for target in env.target().compile_targets() {
            let arch_dir = platform_dir.join(target.arch().to_string());
            let mut cargo = env.cargo_build(target, &arch_dir.join("cargo"))?;
            if !bin_target {
                cargo.arg("--lib");
            }
            cargo.exec()?;
        }
        runner.end_verbose_task();
    }

    runner.start_task(format!("Create {}", env.target().format()));
    match env.target().platform() {
        Platform::Linux => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());

            let appimage = AppImage::new(&arch_dir, env.name().to_string())?;
            appimage.add_apprun()?;
            appimage.add_desktop()?;
            if let Some(icon) = env.icon() {
                appimage.add_icon(icon)?;
            }

            let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
            appimage.add_file(&main, Path::new(env.name()))?;

            if has_lib {
                let lib = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Cdylib)?;
                appimage.add_file(&lib, &Path::new("lib").join(lib.file_name().unwrap()))?;
            }

            if env.target().format() == Format::Appimage {
                let out = arch_dir.join(format!("{}.AppImage", env.name()));
                appimage.build(&out, env.target().signer().cloned())?;
            }
        }
        Platform::Android => {
            let out = platform_dir.join(format!("{}.{}", env.name(), env.target().format()));
            if env.config().android().gradle {
                crate::gradle::build(env, &out)?;
                runner.end_verbose_task();
                return Ok(());
            } else {
                let mut apk = Apk::new(
                    out,
                    env.config().android().manifest.clone(),
                    env.target().opt() != Opt::Debug,
                )?;
                apk.add_res(env.icon(), &env.android_jar())?;

                if has_lib {
                    for target in env.target().compile_targets() {
                        let arch_dir = platform_dir.join(target.arch().to_string());
                        let cargo_dir = arch_dir.join("cargo");
                        let lib = env.cargo_artefact(&cargo_dir, target, CrateType::Cdylib)?;
                        apk.add_lib(target.android_abi(), &lib)?;

                        let ndk = env.android_ndk();

                        let deps_dir = {
                            let arch_dir = if target.is_host()? {
                                cargo_dir.to_path_buf()
                            } else {
                                cargo_dir.join(target.rust_triple()?)
                            };
                            let opt_dir = arch_dir.join(target.opt().to_string());
                            opt_dir.join("deps")
                        };

                        let mut search_paths = env
                            .cargo()
                            .lib_search_paths(&cargo_dir, target)
                            .with_context(|| {
                                format!(
                                    "Finding libraries in `{}` for {:?}",
                                    cargo_dir.display(),
                                    target
                                )
                            })?;
                        search_paths.push(deps_dir);
                        let search_paths =
                            search_paths.iter().map(AsRef::as_ref).collect::<Vec<_>>();

                        let ndk_sysroot_libs = ndk.join("usr/lib").join(target.ndk_triple());
                        let provided_libs_paths = [
                            ndk_sysroot_libs.as_path(),
                            &*ndk_sysroot_libs.join(
                                // Use libraries (symbols) from the lowest NDK that is supported by the application,
                                // to prevent inadvertently making newer APIs available:
                                // https://developer.android.com/ndk/guides/sdk-versions
                                env.config()
                                    .android()
                                    .manifest
                                    .sdk
                                    .min_sdk_version
                                    .unwrap()
                                    .to_string(),
                            ),
                        ];

                        let extra_libs = xcommon::llvm::list_needed_libs_recursively(
                            &lib,
                            &search_paths,
                            &provided_libs_paths,
                        )
                        .with_context(|| {
                            format!(
                                "Failed to collect all required libraries for `{}` with `{:?}` available libraries and `{:?}` shippable libraries",
                                lib.display(),
                                provided_libs_paths,
                                search_paths
                            )
                        })?;
                        for lib in &extra_libs {
                            apk.add_lib(target.android_abi(), lib)?;
                        }
                    }
                }

                apk.finish(env.target().signer().cloned())?;
            }
        }
        Platform::Macos => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());

            let mut app = AppBundle::new(&arch_dir, env.config().macos().info.clone())?;
            if let Some(icon) = env.icon() {
                app.add_icon(icon)?;
            }

            let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
            app.add_executable(&main)?;

            if has_lib {
                let lib = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Cdylib)?;
                app.add_lib(&lib)?;
            }

            app.finish(env.target().signer().cloned())?;
            if let Some(api_key) = env.target().api_key() {
                appbundle::notarize(app.appdir(), api_key)?;
            }
            if env.target().format() == Format::Dmg {
                let out = arch_dir.join(format!("{}.dmg", env.name()));
                apple_dmg::create_dmg(app.appdir(), &out, env.name(), 0x40000)?;
                if let Some(signer) = env.target().signer() {
                    app.sign_dmg(&out, signer)?;
                    if let Some(api_key) = env.target().api_key() {
                        appbundle::notarize(&out, api_key)?;
                    }
                }
            }
        }
        Platform::Ios => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());
            std::fs::create_dir_all(&arch_dir)?;
            let mut app = AppBundle::new(&arch_dir, env.config().ios().info.clone())?;
            if let Some(icon) = env.icon() {
                app.add_icon(icon)?;
            }
            let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
            app.add_executable(&main)?;
            if let Some(provisioning_profile) = env.target().provisioning_profile() {
                app.add_provisioning_profile(provisioning_profile)?;
            }
            app.finish(env.target().signer().cloned())?;
            if env.target().format() == Format::Ipa {
                let app = arch_dir.join(format!("{}.app", env.name()));
                let out = arch_dir.join(format!("{}.ipa", env.name()));
                let mut ipa = Zip::new(&out, false)?;
                ipa.add_directory(
                    &app,
                    &Path::new("Payload").join(format!("{}.app", env.name())),
                )?;
                ipa.finish()?;
            }
        }
        Platform::Windows => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());
            std::fs::create_dir_all(&arch_dir)?;
            let out = arch_dir.join(format!("{}.{}", env.name(), env.target().format()));
            let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
            match env.target().format() {
                Format::Exe => {
                    std::fs::copy(&main, &out)?;
                }
                Format::Msix => {
                    let mut msix = Msix::new(
                        out,
                        env.config().windows().manifest.clone(),
                        target.opt() != Opt::Debug,
                    )?;
                    if let Some(icon) = env.icon() {
                        msix.add_icon(icon)?;
                    }
                    // TODO: *.pri

                    msix.add_file(
                        &main,
                        format!("{}.exe", env.name()).as_ref(),
                        ZipFileOptions::Compressed,
                    )?;

                    if has_lib {
                        let lib =
                            env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Cdylib)?;
                        msix.add_file(
                            &lib,
                            Path::new(lib.file_name().unwrap()),
                            ZipFileOptions::Compressed,
                        )?;
                    }

                    msix.finish(env.target().signer().cloned())?;
                }
                _ => {
                    anyhow::bail!("unsupported windows format");
                }
            }
        }
    }
    runner.end_task();

    Ok(())
}
