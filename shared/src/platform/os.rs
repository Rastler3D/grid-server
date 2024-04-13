use serde_derive::*;

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum Os{
    Linux,
    Macos,
    Ios,
    Freebsd,
    Dragonfly,
    Netbsd,
    Openbsd,
    Solaris,
    Android,
    Windows,
    Unknown
}

impl Os {
    pub const fn from_str(os: &str) -> Os {
        match os.as_bytes() {
            b"linux" => Os::Linux,
            b"macos" => Os::Macos,
            b"ios" => Os::Ios,
            b"freebsd" => Os::Freebsd,
            b"dragonfly" => Os::Dragonfly,
            b"netbsd" => Os::Netbsd,
            b"openbsd" => Os::Openbsd,
            b"solaris" => Os::Solaris,
            b"android" => Os::Android,
            b"windows" => Os::Windows,
            _ => Os::Unknown
        }
    }
}