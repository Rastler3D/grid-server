use std::fmt::{Debug, Formatter};

use pyo3::{Py, PyAny, PyResult, Python};
use pyo3::prelude::{PyAnyMethods, PyModule};
use pyo3::types::PyBytes;


const REDUCER: &str = include_str!("../job/reducer.py");

pub fn reduce(job_data: Vec<u8>, results: Vec<Vec<u8>>) -> Result<String, anyhow::Error>{
    let output = Python::with_gil(|py| -> PyResult<_>{
        let module = PyModule::from_code_bound(py, REDUCER, "reducer.py", "reducer")?;
        let splitter = module.getattr("reducer")?;
        let result = splitter.call((PyBytes::new(py,&job_data), results.iter().map(|data| PyBytes::new(py,&data),).collect::<Vec<_>>()), None)?.extract::<(String)>()?;
        Ok(result)
    })?;

    Ok(output)
}