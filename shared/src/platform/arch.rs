use serde_derive::*;

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Hash, Serialize, Deserialize)]
pub enum Arch{
    Wasm32,
    X86,
    X86_64,
    Arm,
    Aarch64,
    Loongarch64,
    M68k,
    Csky,
    Mips,
    Mips64,
    Powerpc,
    Powerpc64,
    Riscv64,
    S390x,
    Sparc64,
    Unknown
}

impl Arch{
    pub const fn from_str(arch: &str) -> Arch{
        match arch.as_bytes() {
            b"wasm32" => Arch::Wasm32,
            b"x86" => Arch::X86,
            b"x86_64" => Arch::X86_64,
            b"arm" => Arch::Arm,
            b"aarch64" => Arch::Aarch64,
            b"loongarch64" => Arch::Loongarch64,
            b"m68k" => Arch::M68k,
            b"csky" => Arch::Csky,
            b"mips" => Arch::Mips,
            b"mips64" => Arch::Mips64,
            b"powerpc" => Arch::Powerpc,
            b"powerpc64" => Arch::Powerpc64,
            b"riscv64" => Arch::Riscv64,
            b"s390x" => Arch::S390x,
            b"sparc64" => Arch::Sparc64,
            _ => Arch::Unknown
        }
    }
}