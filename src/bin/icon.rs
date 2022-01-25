use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    icon: PathBuf,
    #[clap(short, long)]
    res: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    //xstore::apk::mipmap::mipmap_ic_launcher(&args.icon, &args.res)?;
    Ok(())
}
