use crate::Opt;
use anyhow::Result;
use pubgrub::error::PubGrubError;
use pubgrub::range::Range;
use pubgrub::report::{DefaultStringReporter, Reporter};
use pubgrub::solver::OfflineDependencyProvider;
use pubgrub::version::SemanticVersion;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use xapk::Target;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct Package {
    repo: Option<String>,
    #[serde(rename = "$unflatten=groupId")]
    group: String,
    #[serde(rename = "$unflatten=artifactId")]
    name: String,
}

impl Package {
    pub fn new(group: String, name: String) -> Self {
        Self {
            repo: None,
            group,
            name,
        }
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.group, self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename = "dependency")]
pub struct Dependency {
    repo: Option<String>,
    #[serde(rename = "$unflatten=groupId")]
    group: String,
    #[serde(rename = "$unflatten=artifactId")]
    name: String,
    #[serde(rename = "$unflatten=version")]
    version: String,
    #[serde(rename = "$unflatten=scope")]
    scope: Option<String>,
}

impl Dependency {
    pub fn new(package: Package, version: String) -> Self {
        Self {
            repo: package.repo,
            group: package.group,
            name: package.name,
            version,
            scope: None,
        }
    }

    pub fn flutter_embedding(opt: Opt, engine_version: &str) -> Self {
        Self {
            repo: Some("http://download.flutter.io".into()),
            group: "io.flutter".into(),
            name: format!("flutter_embedding_{}", opt),
            version: format!("1.0.0-{}", engine_version),
            scope: None,
        }
    }

    pub fn flutter_engine(target: Target, opt: Opt, engine_version: &str) -> Self {
        let target = match target {
            Target::Arm64V8a => "arm64_v8a",
            Target::ArmV7a => "armeabi_v7a",
            Target::X86 => "x86",
            Target::X86_64 => "x86_64",
        };
        Self {
            repo: Some("http://download.flutter.io".into()),
            group: "io.flutter".into(),
            name: format!("{}_{}", target, opt),
            version: format!("1.0.0-{}", engine_version),
            scope: None,
        }
    }

    pub fn package(&self) -> Package {
        Package {
            repo: self.repo.clone(),
            group: self.group.clone(),
            name: self.name.clone(),
        }
    }

    pub fn version(&self) -> Result<Version> {
        let (v, s) = self
            .version
            .split_once('-')
            .unwrap_or_else(|| (self.version.as_str(), ""));
        Ok(Version {
            version: v.parse()?,
            suffix: s.to_string(),
        })
    }

    pub fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }

    pub fn range(&self) -> Range<Version> {
        let ((major, minor, patch), suffix) = if let Ok(version) = self.version() {
            (version.version.into(), version.suffix)
        } else {
            return Range::any();
        };
        if major < 1 {
            if minor < 1 {
                Range::exact(Version::new((major, minor, patch).into(), suffix))
            } else {
                Range::between(
                    Version::new((major, minor, patch).into(), suffix.clone()),
                    Version::new((major, minor + 1, 0).into(), suffix),
                )
            }
        } else {
            Range::between(
                Version::new((major, minor, patch).into(), suffix.clone()),
                Version::new((major + 1, 0, 0).into(), suffix),
            )
        }
    }

    pub fn file_name(&self, ext: &str) -> Result<String> {
        Ok(format!(
            "{}-{}-{}.{}",
            self.group,
            self.name,
            self.version()?,
            ext
        ))
    }
}

impl std::str::FromStr for Dependency {
    type Err = anyhow::Error;

    fn from_str(dep: &str) -> Result<Self> {
        if let Some((group, dep)) = dep.split_once(':') {
            if let Some((name, version)) = dep.split_once(':') {
                return Ok(Self::new(
                    Package::new(group.into(), name.into()),
                    version.into(),
                ));
            }
        }
        anyhow::bail!("invalid dependency string, expected `group:name:version`");
    }
}

impl std::fmt::Display for Dependency {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.package().group(),
            self.package().name(),
            self.version
        )
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Version {
    version: SemanticVersion,
    suffix: String,
}

impl Version {
    pub fn new(version: SemanticVersion, suffix: String) -> Self {
        Self { version, suffix }
    }
}

impl pubgrub::version::Version for Version {
    fn lowest() -> Self {
        Self {
            version: SemanticVersion::lowest(),
            suffix: "".into(),
        }
    }

    fn bump(&self) -> Self {
        Self {
            version: self.version.bump(),
            suffix: self.suffix.clone(),
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.suffix.is_empty() {
            write!(f, "{}", self.version)
        } else {
            write!(f, "{}-{}", self.version, self.suffix)
        }
    }
}

pub struct Maven {
    client: reqwest::blocking::Client,
    cache_dir: PathBuf,
}

impl Maven {
    pub const GOOGLE: &'static str = "https://maven.google.com";

    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache_dir,
            client: reqwest::blocking::Client::new(),
        })
    }

    pub fn resolve(&self, root: Dependency) -> Result<Vec<PathBuf>> {
        let mut provider = OfflineDependencyProvider::new();
        let mut deps = vec![root.clone()];
        while let Some(dep) = deps.pop() {
            let pom = self.pom(&dep)?;
            provider.add_dependencies(
                dep.package(),
                dep.version()?,
                pom.dependencies().map(|dep| (dep.package(), dep.range())),
            );
            deps.extend(pom.dependencies().cloned());
        }
        pubgrub::solver::resolve(&provider, root.package(), root.version()?)
            .map_err(|err| {
                if let PubGrubError::NoSolution(mut tree) = err {
                    tree.collapse_no_versions();
                    anyhow::anyhow!("{}", DefaultStringReporter::report(&tree))
                } else {
                    anyhow::anyhow!("{}", err)
                }
            })?
            .into_iter()
            .map(|(p, v)| self.package(&Dependency::new(p, v.to_string())))
            .collect()
    }

    /*fn metadata(&self, dep: &Dependency) -> Result<Metadata> {
        let url = format!(
            "{repo}/{group}/{name}/maven-metadata.xml",
            repo = dep.repo.as_deref().unwrap_or(Self::GOOGLE),
            group = dep.package().group().replace('.', "/"),
            name = dep.package().name(),
        );
        let file_name = dep.package().file_name()?;
        let path = self.cache(&url, &file_name)?;
        let r = BufReader::new(File::open(path)?);
        Ok(quick_xml::de::from_reader(r)?)
    }*/

    fn pom(&self, dep: &Dependency) -> Result<Pom> {
        let path = self.artefact(dep, "pom")?;
        let s = std::fs::read_to_string(path)?;
        let pom = quick_xml::de::from_str(&s).map_err(|err| anyhow::anyhow!("{}: {}", err, s))?;
        Ok(pom)
    }

    pub fn package(&self, dep: &Dependency) -> Result<PathBuf> {
        self.artefact(dep, self.pom(dep)?.packaging())
    }

    fn artefact(&self, dep: &Dependency, ext: &str) -> Result<PathBuf> {
        let url = format!(
            "{repo}/{group}/{name}/{version}/{name}-{version}.{ext}",
            repo = dep.repo.as_deref().unwrap_or(Self::GOOGLE),
            group = dep.package().group().replace('.', "/"),
            name = dep.package().name(),
            version = dep.version()?,
            ext = ext,
        );
        let file_name = dep.file_name(ext)?;
        self.cache(&url, &file_name)
    }

    fn cache(&self, url: &str, file_name: &str) -> Result<PathBuf> {
        let path = self.cache_dir.join(file_name);
        if !path.exists() {
            println!("downloading {}", url);
            let resp = self.client.get(url).send()?;
            if !resp.status().is_success() {
                anyhow::bail!("GET {} returned status code {}", url, resp.status());
            }
            let mut r = BufReader::new(resp);
            let mut w = BufWriter::new(File::create(&path)?);
            std::io::copy(&mut r, &mut w)?;
        }
        Ok(path)
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename = "dependencies")]
struct Dependencies {
    #[serde(rename = "$value")]
    #[serde(default)]
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename = "project")]
pub struct Pom {
    packaging: Option<String>,
    #[serde(default)]
    dependencies: Dependencies,
}

impl Pom {
    pub fn packaging(&self) -> &str {
        if let Some(s) = self.packaging.as_deref() {
            s
        } else {
            "jar"
        }
    }

    pub fn dependencies(&self) -> impl Iterator<Item = &Dependency> + '_ {
        self.dependencies
            .dependencies
            .iter()
            .filter(|dep| dep.scope() != Some("test"))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename = "metadata")]
pub struct Metadata {
    #[serde(rename = "$value")]
    versioning: Versioning,
}

impl Metadata {
    pub fn versions(&self) -> &[String] {
        &self.versioning.versions.versions
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename = "versioning")]
struct Versioning {
    #[serde(rename = "$value")]
    versions: Versions,
}

#[derive(Deserialize, Serialize)]
#[serde(rename = "versions")]
struct Versions {
    #[serde(rename = "$unflatten=version")]
    #[serde(default)]
    versions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dep() -> Result<()> {
        let dep = r#"
            <dependency>
                <groupId>group</groupId>
                <artifactId>name</artifactId>
                <version>1.0.0-alpha</version>
            </dependency>"#;
        let dep: Dependency = quick_xml::de::from_str(dep)?;
        assert_eq!(dep.package().group(), "group");
        assert_eq!(dep.package().name(), "name");
        assert_eq!(dep.version()?.to_string(), "1.0.0-alpha");
        Ok(())
    }

    #[test]
    fn test_pom() -> Result<()> {
        let pom = r#"
            <project>
                <dependencies>
                    <dependency>
                        <groupId>group</groupId>
                        <artifactId>name</artifactId>
                        <version>0.0.1</version>
                    </dependency>
                </dependencies>
            </project>"#;
        let pom: Pom = quick_xml::de::from_str(pom)?;
        let deps = pom.dependencies().collect::<Vec<_>>();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].package().group(), "group");
        assert_eq!(deps[0].package().name(), "name");
        assert_eq!(deps[0].version()?.to_string(), "0.0.1");
        Ok(())
    }

    #[test]
    fn test_pom2() -> Result<()> {
        let pom = r#"
            <project>
                <dependencies/>
            </project>"#;
        let pom: Pom = quick_xml::de::from_str(pom)?;
        let deps = pom.dependencies().collect::<Vec<_>>();
        assert!(deps.is_empty());
        Ok(())
    }

    #[test]
    fn test_metadata() -> Result<()> {
        let meta = r#"
            <metadata>
                <versioning>
                    <versions>
                        <version>a</version>
                    </versions>
                </versioning>
            </metadata>"#;
        let meta: Metadata = quick_xml::de::from_str(meta)?;
        assert_eq!(meta.versions().len(), 1);
        assert_eq!(meta.versions()[0], "a");
        Ok(())
    }

    #[test]
    fn test_resolve_flutter() -> Result<()> {
        let flutter = crate::sdk::flutter::Flutter::from_env()?;
        let flutter_dep = Dependency::flutter_embedding(Opt::Debug, &flutter.engine_version()?);
        let mvn = Maven::new("/tmp/test_resolve_flutter".into())?;
        let deps = mvn.resolve(flutter_dep)?;
        for dep in deps {
            println!("{}", dep.display());
        }
        Ok(())
    }
}
