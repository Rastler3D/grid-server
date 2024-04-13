mod path_finding;

#[cfg(target_arch = "wasm32")]
mod bindings;
#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::bindings::{self, Guest};
    use crate::path_finding::job;
    bindings::export!(Component with_types_in bindings);
    struct Component;

    impl Guest for Component {
        fn execute_job(job_data: Vec<u8>, task_data: Vec<u8>) -> Result<Vec<u8>, String> {
            job(job_data,task_data)
        }
    }


}

#[cfg(not(target_arch = "wasm32"))]
mod dll{
    use crate::path_finding::job;

    #[no_mangle]
    pub extern fn execute_job(job_data: Vec<u8>, task_data: Vec<u8>) -> Result<Vec<u8>, String>{
        job(job_data,task_data)
    }
}