use pyo3::prelude::{PyAnyMethods, PyModule};
use pyo3::{Py, PyResult, Python};
use pyo3::types::PyIterator;

const JOB_EXTRACTOR: &str = include_str!("../job/job.py");

pub fn extract_job(data: &str) -> Result<Vec<u8>, anyhow::Error>{
    let result = Python::with_gil(|py| -> PyResult<_> {
        let module = PyModule::from_code_bound(py, JOB_EXTRACTOR, "job.py", "job")?;
        let splitter = module.getattr("extract_job")?;
        let result = splitter.call((data,), None)?.extract::<(Vec<u8>)>()?;

        Ok(result)
    })?;

    Ok(result)

}