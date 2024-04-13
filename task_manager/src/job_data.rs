use shared::platform::arch::Arch;
use shared::platform::family::Family;
use shared::platform::os::Os;
use shared::platform::Platform;
pub const JOB_DATA: &[(Platform, &[u8])] = &[
    (Platform{ arch: Arch::X86_64, family: Family::Unix, os: Os::Linux }, include_bytes!("../job/target/x86_64-unknown-linux-gnu/release/libjob.so")),
    (Platform{ arch: Arch::X86_64, family: Family::Windows, os: Os::Windows }, include_bytes!("../job/target/x86_64-pc-windows-msvc/release/job.dll")),
    (Platform{ arch: Arch::Wasm32, family: Family::Unknown, os: Os::Unknown }, include_bytes!("../job/target/wasm32-wasi/release/job.wasm"))
];