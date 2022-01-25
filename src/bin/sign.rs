use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    input: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    //xstore::apk::read_zip(&args.input)?;
    //xstore::msix::p7x::read_p7x(&args.input)?;
    Ok(())
}
