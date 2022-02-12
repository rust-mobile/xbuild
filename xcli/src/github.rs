use anyhow::Result;
use std::path::Path;
use tar::Archive;
use zstd::Decoder;

pub fn download_tar_zst(
    out: &Path,
    org: &str,
    repo: &str,
    version: &str,
    artifact: &str,
) -> Result<()> {
    let url = format!(
        "https://github.com/{}/{}/releases/download/{}/{}",
        org, repo, version, artifact
    );
    let client = reqwest::blocking::Client::new();
    let resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("GET {} returned status code {}", url, resp.status());
    }
    Archive::new(Decoder::new(resp)?).unpack(out)?;
    Ok(())
}
