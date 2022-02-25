use crate::metadata::Metadata;
use crate::package::Artifact;
use crate::pom::Pom;
use anyhow::Result;
use pubgrub::error::PubGrubError;
use pubgrub::range::Range;
use pubgrub::report::{DefaultStringReporter, Reporter};
use pubgrub::solver::{Dependencies, DependencyProvider};
use std::borrow::Borrow;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod metadata;
mod package;
mod pom;
mod range;

pub use package::{Package, Version};

pub struct Maven {
    client: reqwest::blocking::Client,
    cache_dir: PathBuf,
    repositories: Vec<&'static str>,
}

impl Maven {
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache_dir,
            client: reqwest::blocking::Client::new(),
            repositories: vec![],
        })
    }

    pub fn add_repository(&mut self, repo: &'static str) {
        self.repositories.push(repo);
    }

    pub fn resolve(&self, package: Package, version: Version) -> Result<Vec<PathBuf>> {
        pubgrub::solver::resolve(self, package, version)
            .map_err(|err| {
                if let PubGrubError::NoSolution(mut tree) = err {
                    tree.collapse_no_versions();
                    anyhow::anyhow!("{}", DefaultStringReporter::report(&tree))
                } else {
                    anyhow::anyhow!("{:?}", err)
                }
            })?
            .into_iter()
            .map(|(package, version)| self.package(&package, &version))
            .collect()
    }

    pub fn package(&self, package: &Package, version: &Version) -> Result<PathBuf> {
        let artifact = Artifact { package, version };
        let pom = self.pom(artifact)?;
        self.artifact(artifact, pom.packaging())
    }

    fn versions(&self, package: &Package, range: &Range<Version>) -> Vec<Version> {
        match self.metadata(package) {
            Ok(metadata) => metadata
                .versions()
                .into_iter()
                .filter_map(|version| Version::from_str(version).ok())
                .filter(|version| range.contains(&version))
                .rev()
                .collect(),
            Err(err) => {
                log::debug!("failed to get metadata for {}: {}", package, err);
                range.lowest_version().into_iter().collect()
            }
        }
    }

    fn metadata(&self, package: &Package) -> Result<Metadata> {
        let path = self.cache_dir.join(package.file_name());
        if !path.exists() {
            let mut downloaded = false;
            for repo in &self.repositories {
                let url = package.url(repo);
                if self.download(&url, &path).is_ok() {
                    downloaded = true;
                    break;
                }
            }
            if !downloaded {
                anyhow::bail!("metadata not found for {}", package);
            }
        }
        let s = std::fs::read_to_string(path)?;
        let metadata =
            quick_xml::de::from_str(&s).map_err(|err| anyhow::anyhow!("{}: {}", err, s))?;
        Ok(metadata)
    }

    fn pom(&self, artifact: Artifact) -> Result<Pom> {
        let path = self.artifact(artifact, "pom")?;
        let s = std::fs::read_to_string(path)?;
        let pom = quick_xml::de::from_str(&s).map_err(|err| anyhow::anyhow!("{}: {}", err, s))?;
        Ok(pom)
    }

    fn artifact(&self, artifact: Artifact, ext: &str) -> Result<PathBuf> {
        let path = self.cache_dir.join(artifact.file_name(ext));
        if !path.exists() {
            log::info!("downloading {}", artifact);
            let mut downloaded = false;
            for repo in &self.repositories {
                let url = artifact.url(repo, ext);
                if self.download(&url, &path).is_ok() {
                    downloaded = true;
                    break;
                }
            }
            if !downloaded {
                anyhow::bail!("artifact not found {}", artifact);
            }
        }
        Ok(path)
    }

    fn download(&self, url: &str, path: &Path) -> Result<()> {
        log::debug!("get {}", url);
        let resp = self.client.get(url).send()?;
        if !resp.status().is_success() {
            anyhow::bail!("GET {} returned status code {}", url, resp.status());
        }
        let mut r = BufReader::new(resp);
        let mut w = BufWriter::new(File::create(&path)?);
        std::io::copy(&mut r, &mut w)?;
        Ok(())
    }
}

impl DependencyProvider<Package, Version> for Maven {
    fn choose_package_version<T: Borrow<Package>, U: Borrow<Range<Version>>>(
        &self,
        potential_packages: impl Iterator<Item = (T, U)>,
    ) -> Result<(T, Option<Version>), Box<dyn Error>> {
        let mut selected: Option<(_, Vec<_>)> = None;
        for (p, r) in potential_packages {
            let versions = self.versions(p.borrow(), r.borrow());
            if let Some((_, v)) = &selected {
                if v.len() < versions.len() {
                    continue;
                }
            }
            let early_exit = versions.len() < 2;
            selected = Some((p, versions));
            if early_exit {
                break;
            }
        }
        let (p, v) = selected.expect("non empty iterator");
        let v = v.into_iter().next();
        log::debug!("chose {} {:?}", p.borrow(), v);
        Ok((p, v))
    }

    fn get_dependencies(
        &self,
        package: &Package,
        version: &Version,
    ) -> Result<Dependencies<Package, Version>, Box<dyn Error>> {
        //println!("get dependencies {} {}", package, version);
        let pom = self.pom(Artifact { package, version }).unwrap();
        let deps = pom
            .dependencies()
            .iter()
            .filter(|dep| dep.scope().is_none() || dep.scope() == Some("compile"))
            .map(|dep| Ok((dep.package(), dep.range().unwrap())))
            .collect::<Result<_>>()?;
        log::debug!("{} {} has deps {:?}", package, version, deps);
        Ok(Dependencies::Known(deps))
    }
}
