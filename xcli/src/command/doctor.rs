#[derive(Clone, Debug)]
pub struct Doctor {
    groups: Vec<Group>,
}

impl Default for Doctor {
    fn default() -> Self {
        Self {
            groups: vec![
                Group {
                    name: "clang/llvm toolchain",
                    checks: vec![
                        "clang",
                        "clang++",
                        "llvm-ar",
                        "llvm-lib",
                        "lld",
                        "lld-link",
                        "lldb",
                        "lldb-server",
                    ],
                },
                Group {
                    name: "misc",
                    checks: vec!["cargo", "git", "flutter"],
                },
                Group {
                    name: "android",
                    checks: vec!["adb", "javac", "java"],
                },
                Group {
                    name: "ios",
                    checks: vec![
                        "idevice_id",
                        "ideviceinfo",
                        "ideviceinstaller",
                        "ideviceimagemounter",
                        "idevicedebug",
                    ],
                },
            ],
        }
    }
}

impl std::fmt::Display for Doctor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for group in &self.groups {
            write!(f, "{}", group)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct Group {
    name: &'static str,
    checks: Vec<&'static str>,
}

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "{:-^1$}", self.name, 60)?;
        for check in &self.checks {
            write!(f, "{:20} ", check)?;
            if let Ok(path) = which::which(check) {
                write!(f, "{}", path.display())?;
            } else {
                write!(f, "not found")?;
            }
            writeln!(f, "")?;
        }
        writeln!(f, "")
    }
}

pub fn doctor() {
    let doctor = Doctor::default();
    print!("{}", doctor);
}
