use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[repr(u8)]
pub enum Target {
    #[serde(rename = "armv7-linux-androideabi")]
    ArmV7a = 1,
    #[serde(rename = "aarch64-linux-android")]
    Arm64V8a = 2,
    #[serde(rename = "i686-linux-android")]
    X86 = 3,
    #[serde(rename = "x86_64-linux-android")]
    X86_64 = 4,
}

impl Target {
    /// Identifier used in the NDK to refer to the ABI
    pub fn android_abi(self) -> &'static str {
        match self {
            Self::Arm64V8a => "arm64-v8a",
            Self::ArmV7a => "armeabi-v7a",
            Self::X86 => "x86",
            Self::X86_64 => "x86_64",
        }
    }
}
