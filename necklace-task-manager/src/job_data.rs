use shared::platform::arch::Arch;
use shared::platform::family::Family;
use shared::platform::os::Os;
use shared::platform::Platform;
pub const JOB_DATA: &[(Platform, &[u8])] = &[
    (Platform{ arch: Arch::Wasm32, family: Family::Unknown, os: Os::Unknown }, include_bytes!("../job/executor.wasm"))
];