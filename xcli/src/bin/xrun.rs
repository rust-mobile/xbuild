use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use xcli::devices::Device;

#[derive(Parser)]
pub struct Args {
    #[clap(long)]
    path: PathBuf,
    #[clap(long)]
    device: Option<Device>,
    #[clap(long, requires = "activity", conflicts_with = "bundle_id")]
    package: Option<String>,
    #[clap(long, requires = "package", conflicts_with = "bundle_id")]
    activity: Option<String>,
    #[clap(long)]
    bundle_id: Option<String>,
}

fn main() -> Result<()> {
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
    let device = args.device.unwrap_or_else(Device::host);
    let attach = true;
    let run = match (args.package, args.activity, args.bundle_id) {
        (Some(package), Some(activity), _) => {
            device.xrun_adb(&args.path, &package, &activity, attach)?
        }
        (_, _, Some(_bundle_id)) => {
            todo!()
        }
        _ => device.xrun_host(&args.path, attach)?,
    };
    if let Some(url) = run.url {
        println!("found url {}", url);
    }
    if let Some(mut child) = run.child {
        child.kill()?;
    }
    Ok(())
}
