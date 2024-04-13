use serde_derive::*;
use std::env;
use arch::Arch;
use family::Family;
use os::Os;

pub mod arch;
pub mod family;
pub mod os;

pub const WASM_PLATFORM: Platform = Platform::from_str("wasm32", "", "");
pub const CURRENT_PLATFORM: Platform = Platform::from_str(env::consts::ARCH, env::consts::FAMILY,env::consts::OS);

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct Platform{
    pub arch: Arch,
    pub family: Family,
    pub os: Os
}

impl Platform {
    pub const fn from_str(arch: &str, family: &str, os: &str) -> Platform{
        Platform{
            arch: Arch::from_str(arch),
            family: Family::from_str(family),
            os: Os::from_str(os)
        }
    }
}

