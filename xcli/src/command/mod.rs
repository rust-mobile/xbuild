use crate::cargo::CrateType;
use crate::devices::Device;
use crate::task::TaskRunner;
use crate::{BuildEnv, CompileTarget};
use anyhow::Result;
use std::process::Command;

//mod attach;
mod build;
mod doctor;
mod new;

//pub use attach::attach;
pub use build::build;
pub use doctor::doctor;
pub use new::new;

pub fn devices() -> Result<()> {
    for device in Device::list()? {
        println!(
            "{:50}{:20}{:20}{}",
            device.to_string(),
            device.name()?,
            format!("{} {}", device.platform()?, device.arch()?),
            device.details()?,
        );
    }
    Ok(())
}

pub fn update(env: &BuildEnv) -> Result<()> {
    let mut runner = TaskRunner::new(3, env.verbose());

    runner.start_task("Update flutter");
    if let Some(flutter) = env.flutter() {
        flutter.git_pull()?;
        runner.end_verbose_task();
    }

    runner.start_task("Run pub upgrade");
    if let Some(flutter) = env.flutter() {
        flutter.pub_upgrade(env.root_dir())?;
        runner.end_verbose_task();
    }

    runner.start_task("Run cargo update");
    Command::new("cargo")
        .current_dir(env.root_dir())
        .arg("update")
        .status()?;
    runner.end_verbose_task();

    Ok(())
}

pub fn run(env: &BuildEnv) -> Result<()> {
    let out = env.executable();
    if let Some(device) = env.target().device() {
        device.run(&out, env, env.has_dart_code())?;
    } else {
        anyhow::bail!("no device specified");
    }
    Ok(())
}

pub fn lldb(env: &BuildEnv) -> Result<()> {
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
