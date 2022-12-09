use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use xbuild::{command, BuildArgs, BuildEnv};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

fn main() -> Result<()> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_log::LogTracer::init().ok();
    let env = std::env::var("XBUILD_LOG").unwrap_or_else(|_| "error".into());
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
    /// Creates a new rust project
    New {
        /// Project name
        name: String,
    },
    /// Show information about the installed tooling
    Doctor,
    /// List all connected devices
    Devices,
    /// Build an executable app or install bundle
    Build {
        #[clap(flatten)]
        args: BuildArgs,
    },
    /// Run app on an attached device
    Run {
        #[clap(flatten)]
        args: BuildArgs,
    },
    /// Launch app in a debugger on an attached device
    Lldb {
        #[clap(flatten)]
        args: BuildArgs,
    },
    /// Generates a PEM encoded RSA2048 signing key
    GenerateKey {
        /// Path to write a new PEM encoded RSA2048 signing key
        pem: PathBuf,
    },
    /// Generates a PEM encoded certificate signing request
    GenerateCsr {
        /// Path to a PEM encoded RSA2048 signing key
        pem: PathBuf,
        /// Path to write PEM encoded certificate signing request
        csr: PathBuf,
    },
    /// Adds a certificate to a PEM encoded RSA2048 signing key
    AddCertificate {
        /// Path to a PEM encoded RSA2048 signing key
        pem: PathBuf,
        /// Path to a certificate
        cer: PathBuf,
    },
}

impl Commands {
    pub fn run(self) -> Result<()> {
        match self {
            Self::New { name } => command::new(&name)?,
            Self::Doctor => command::doctor(),
            Self::Devices => command::devices()?,
            Self::Build { args } => {
                let env = BuildEnv::new(args)?;
                command::build(&env)?;
            }
            Self::Run { args } => {
                let env = BuildEnv::new(args)?;
                command::build(&env)?;
                command::run(&env)?;
            }
            Self::Lldb { args } => {
                let env = BuildEnv::new(args)?;
                command::build(&env)?;
                command::lldb(&env)?;
            }
            Self::GenerateKey { pem } => command::generate_key(&pem)?,
            Self::GenerateCsr { pem, csr } => command::generate_csr(&pem, &csr)?,
            Self::AddCertificate { pem, cer } => command::add_certificate(&pem, &cer)?,
        }
        Ok(())
    }
}
