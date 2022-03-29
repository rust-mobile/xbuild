use crate::cargo::CrateType;
use crate::download::DownloadManager;
use crate::flutter::depfile::depfile_is_dirty;
use crate::task::TaskRunner;
use crate::{BuildEnv, Format, Opt, Platform};
use anyhow::Result;
use appbundle::AppBundle;
use std::path::Path;
use xapk::Apk;
use xappimage::AppImage;
use xcommon::ZipFileOptions;
use xmsix::Msix;

pub fn build(env: &BuildEnv) -> Result<()> {
    let platform_dir = env.platform_dir();
    std::fs::create_dir_all(&platform_dir)?;

    let mut num_tasks = 8;
    let mut build_classes_dex = false;
    if env.target().platform() == Platform::Android {
        num_tasks += 1;
        if !platform_dir.join("classes.dex").exists() {
            build_classes_dex = true;
        }
    }

    let mut runner = TaskRunner::new(num_tasks, env.verbose());

    runner.start_task("Fetch flutter repo");
    if let Some(flutter) = env.flutter() {
        if !flutter.root().exists() {
            if !env.offline() {
                flutter.clone()?;
                runner.end_task();
            } else {
                anyhow::bail!("flutter repo missing. Run `x` without `--offline`");
            }
        }
    }

    // if engine version changed clean

    runner.start_task("Fetch precompiled artefacts");
    let manager = DownloadManager::new(&env)?;
    if !env.offline() {
        manager.prefetch(build_classes_dex)?;
        runner.end_verbose_task();
    }

    runner.start_task("Run pub get");
    if let Some(flutter) = env.flutter() {
        if !env.offline() {
            let package_config = env
                .root_dir()
                .join(".dart_tool")
                .join("package_config.json");
            let pubspec_stamp = env.build_dir().join("pubspec.stamp");
            if !package_config.exists() || xcommon::is_stamp_dirty(env.pubspec(), &pubspec_stamp)? {
                flutter.pub_get(env.root_dir())?;
                xcommon::create_stamp(&pubspec_stamp)?;
                runner.end_task();
            }
        }
    }

    runner.start_task("Build rust");
    let bin_target = env.target().platform() != Platform::Android
        && (env.flutter().is_some() || env.target().platform() != Platform::Ios);
    let has_lib = env.root_dir().join("src").join("lib.rs").exists();
    if bin_target || has_lib {
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

    if env.target().platform() == Platform::Android {
        runner.start_task("Build classes.dex");
        if let Some(flutter) = env.flutter() {
            if !platform_dir.join("classes.dex").exists() {
                let r8 = manager.r8()?;
                let deps = manager.flutter_embedding()?;
                flutter.build_classes_dex(&env, &r8, deps)?;
                runner.end_task();
            }
        }
    }

    runner.start_task("Build flutter assets");
    if let Some(flutter) = env.flutter() {
        flutter.build_flutter_assets(env.root_dir(), &env.build_dir().join("flutter_assets"))?;
        runner.end_task();
    }

    runner.start_task("Build kernel_blob.bin");
    let mut aot_snapshot = false;
    let kernel_blob = if let Some(flutter) = env.flutter() {
        let kernel_blob = platform_dir.join("kernel_blob.bin");
        let kernel_blob_d = platform_dir.join("kernel_blob.bin.d");
        if !kernel_blob_d.exists() || depfile_is_dirty(&kernel_blob_d)? {
            aot_snapshot = true;
            flutter.kernel_blob_bin(
                env.root_dir(),
                env.target_file(),
                &kernel_blob,
                &kernel_blob_d,
                env.target().opt(),
            )?;
            runner.end_task();
        }
        kernel_blob
    } else {
        Default::default()
    };

    runner.start_task("Build aot snapshot");
    if let Some(flutter) = env.flutter() {
        if env.target().opt() == Opt::Release {
            for target in env.target().compile_targets() {
                let arch_dir = platform_dir.join(target.arch().to_string());
                let output =
                    if target.platform() == Platform::Macos || target.platform() == Platform::Ios {
                        arch_dir.join("App.framework")
                    } else {
                        arch_dir.join("libapp.so")
                    };
                if !output.exists() || aot_snapshot {
                    std::fs::create_dir_all(&arch_dir)?;
                    flutter.aot_snapshot(env.root_dir(), &kernel_blob, &output, target)?;
                }
            }
            runner.end_task();
        }
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
            let out = platform_dir.join(format!("{}.apk", env.name()));
            let mut apk = Apk::new(out.clone(), env.manifest().android().clone())?;
            apk.add_res(env.icon(), &env.android_jar())?;
            if let Some(flutter) = env.flutter() {
                for target in env.target().compile_targets() {
                    apk.add_lib(
                        target.android_abi(),
                        &flutter.engine_dir(target)?.join("libflutter.so"),
                    )?;
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
        }
        Platform::Macos => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());

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
                        app.add_framework(&arch_dir.join("App.framework"))?;
                    }
                }
            }

            let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
            app.add_executable(&main)?;

            if has_lib {
                let lib = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Cdylib)?;
                app.add_lib(&lib)?;
            }

            let appdir = app.finish(env.target().signer().cloned())?;
            if env.target().format() == Format::Dmg {
                let out = arch_dir.join(format!("{}.dmg", env.name()));
                appbundle::make_dmg(&arch_dir, &appdir, &out)?;
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
                        app.add_framework(&arch_dir.join("App.framework"))?;
                    }
                }
                flutter.build_ios_main(&env, target)?;
                app.add_executable(&arch_dir.join("main"))?;
            } else {
                let main = env.cargo_artefact(&arch_dir.join("cargo"), target, CrateType::Bin)?;
                app.add_executable(&main)?;
            }
            app.add_provisioning_profile(env.target().provisioning_profile().unwrap())?;
            app.finish(env.target().signer().cloned())?;
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
        }
    }
    runner.end_task();

    Ok(())
}
