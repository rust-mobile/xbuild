use crate::metadata::Metadata;
use crate::package::Artifact;
use crate::pom::{Dependency, Pom};
use anyhow::{Context, Result};
use pubgrub::{
    DefaultStringReporter, Dependencies, DependencyConstraints, DependencyProvider,
    PackageResolutionStatistics, PubGrubError, Ranges, Reporter,
};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod metadata;
mod package;
mod pom;
mod range;

pub use package::{Package, Version};

pub trait Download {
    fn download(&self, url: &str, dest: &Path) -> Result<()>;
}

pub struct Maven<D: Download> {
    client: D,
    cache_dir: PathBuf,
    repositories: Vec<&'static str>,
    local: HashMap<(Package, Version), Dependencies<Package, Ranges<Version>, String>>,
}

impl<D: Download> Maven<D> {
    pub fn new(cache_dir: PathBuf, client: D) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache_dir,
            client,
            repositories: vec![],
            local: Default::default(),
        })
    }

    pub fn add_repository(&mut self, repo: &'static str) {
        self.repositories.push(repo);
    }

    pub fn add_package(&mut self, package: Package, version: Version, deps: Vec<Dependency>) {
        let deps = deps
            .into_iter()
            .map(|dep| (dep.package(), dep.range().unwrap()))
            .collect();
        self.local
            .insert((package, version), Dependencies::Available(deps));
    }

    pub fn resolve(&self, package: Package, version: Version) -> Result<Vec<PathBuf>> {
        Ok(pubgrub::resolve(self, package, version)
            .map_err(|err| {
                let PubGrubError::NoSolution(mut tree) = err;
                tree.collapse_no_versions();
                anyhow::anyhow!("{}", DefaultStringReporter::report(&tree))
            })?
            .into_iter()
            .filter_map(
                |(package, version)| match self.package(&package, &version) {
                    Ok(path) => {
                        if let Ok(metadata) = self.metadata(&package) {
                            log::info!(
                                "selected {} {} (latest {}) (release {})",
                                package,
                                version,
                                metadata.latest(),
                                metadata.release(),
                            );
                        } else {
                            log::info!("selected {package} {version}",);
                        }
                        Some(path)
                    }
                    Err(err) => {
                        log::info!("{err}");
                        None
                    }
                },
            )
            .collect())
    }

    pub fn package(&self, package: &Package, version: &Version) -> Result<PathBuf> {
        let artifact = Artifact { package, version };
        let pom = self.pom(artifact)?;
        self.artifact(artifact, pom.packaging())
    }

    fn versions(&self, package: &Package, range: &Ranges<Version>) -> Vec<Version> {
        match self.metadata(package) {
            Ok(metadata) => metadata
                .versions()
                .iter()
                .filter_map(|version| Version::from_str(version).ok())
                .filter(|version| version.suffix.is_none())
                .filter(|version| range.contains(version))
                .rev()
                .collect(),
            Err(err) => {
                log::debug!("failed to get metadata for {package}: {err}");
                range
                    .bounding_range()
                    .and_then(|(lower, _)| match lower {
                        std::ops::Bound::Included(v) | std::ops::Bound::Excluded(v) => {
                            Some(v.clone())
                        }
                        std::ops::Bound::Unbounded => None,
                    })
                    .into_iter()
                    .collect()
            }
        }
    }

    fn metadata(&self, package: &Package) -> Result<Metadata> {
        let path = self.cache_dir.join(package.file_name());
        if !path.exists() {
            let mut downloaded = false;
            for repo in &self.repositories {
                let url = package.url(repo);
                if self.client.download(&url, &path).is_ok() {
                    downloaded = true;
                    break;
                }
            }
            anyhow::ensure!(downloaded, "metadata not found for {}", package);
        }
        let s = std::fs::read_to_string(path)?;
        let metadata = quick_xml::de::from_str(&s).context(s)?;
        Ok(metadata)
    }

    fn pom(&self, artifact: Artifact) -> Result<Pom> {
        match self.artifact(artifact, "pom") {
            Ok(path) => {
                let s = std::fs::read_to_string(path)?;
                let pom = quick_xml::de::from_str(&s).context(s)?;
                Ok(pom)
            }
            Err(err) => {
                log::info!("{err}");
                Ok(Default::default())
            }
        }
    }

    fn artifact(&self, artifact: Artifact, ext: &str) -> Result<PathBuf> {
        let path = self.cache_dir.join(artifact.file_name(ext));
        if !path.exists() {
            log::info!("downloading {artifact}");
            let mut downloaded = false;
            for repo in &self.repositories {
                let url = artifact.url(repo, ext);
                if self.client.download(&url, &path).is_ok() {
                    downloaded = true;
                    break;
                }
            }
            anyhow::ensure!(downloaded, "artifact not found {} {}", artifact, ext);
        }
        if ext == "aar" {
            let jar = self.cache_dir.join(artifact.file_name("jar"));
            if !jar.exists() {
                let classes = xcommon::extract_zip_file(&path, "classes.jar")?;
                std::fs::write(jar, classes)?;
            }
        }
        Ok(path)
    }
}

impl<D: Download> DependencyProvider for Maven<D> {
    type P = Package;
    type V = Version;
    type VS = Ranges<Version>;
    type M = String;
    type Err = Infallible;
    type Priority = (u32, Reverse<usize>);

    fn prioritize(
        &self,
        package: &Package,
        range: &Ranges<Version>,
        package_conflicts_counts: &PackageResolutionStatistics,
    ) -> Self::Priority {
        let version_count = self.versions(package, range).len();
        if version_count == 0 {
            return (u32::MAX, Reverse(0));
        }
        (package_conflicts_counts.conflict_count(), Reverse(version_count))
    }

    fn choose_version(
        &self,
        package: &Package,
        range: &Ranges<Version>,
    ) -> Result<Option<Version>, Infallible> {
        Ok(self.versions(package, range).into_iter().next())
    }

    fn get_dependencies(
        &self,
        package: &Package,
        version: &Version,
    ) -> Result<Dependencies<Package, Ranges<Version>, String>, Infallible> {
        if let Some(deps) = self.local.get(&(package.clone(), version.clone())) {
            return Ok(deps.clone());
        }
        //println!("get dependencies {} {}", package, version);
        let pom = self.pom(Artifact { package, version }).unwrap();
        let deps: DependencyConstraints<Package, Ranges<Version>> = pom
            .dependencies()
            .iter()
            .filter(|dep| dep.scope() != Some("test"))
            .map(|dep| (dep.package(), dep.range().unwrap()))
            .collect();
        //log::debug!("{} {} has deps {:?}", package, version, deps);
        Ok(Dependencies::Available(deps))
    }
}
