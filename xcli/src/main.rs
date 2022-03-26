use anyhow::Result;
use clap::{Parser, Subcommand};
use xcli::{command, BuildArgs, BuildEnv};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

fn main() -> Result<()> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_log::LogTracer::init().ok();
    let env = std::env::var("X_LOG").unwrap_or_else(|_| "error".into());
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_span_events(FmtSpan::ACTIVE | FmtSpan::CLOSE)
        .with_env_filter(EnvFilter::new(env))
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();
    log_panics::init();
    let args = Args::parse();
    args.command.run()
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
    /*Attach {
        #[clap(long)]
        url: String,
        #[clap(long)]
        root_dir: PathBuf,
        #[clap(long)]
        target_file: PathBuf,
        #[clap(long)]
        host_vmservice_port: Option<u16>,
    },*/
}

impl Commands {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Doctor => command::doctor(),
            Self::Devices => command::devices()?,
            Self::New { name } => command::new(&name)?,
            Self::Build { args } => {
                let env = BuildEnv::new(args)?;
                command::build(&env)?;
            }
            Self::Run { args, no_build } => {
                let env = BuildEnv::new(args)?;
                if !no_build {
                    command::build(&env)?;
                }
                command::run(&env)?;
            }
            Self::Lldb { args } => {
                let env = BuildEnv::new(args)?;
                command::build(&env)?;
                command::lldb(&env)?;
            } /*Self::Attach {
                  url,
                  root_dir,
                  target_file,
                  host_vmservice_port,
              } => command::attach(&url, &root_dir, &target_file, host_vmservice_port)?,*/
        }
        Ok(())
    }
}
