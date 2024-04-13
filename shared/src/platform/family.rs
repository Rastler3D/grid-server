use serde_derive::*;

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum Family{
    Unix,
    Windows,
    Unknown
}

impl Family {
    pub const fn from_str(family: &str) -> Family {
        match family.as_bytes() {
            b"windows" => Family::Windows,
            b"unix" => Family::Unix,
            _ => Family::Unknown
        }
    }
}