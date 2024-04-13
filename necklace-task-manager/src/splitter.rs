use std::iter;
use pyo3::{Py, PyResult, Python};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyIterator, PyModule};

const SPLITTER: &str = include_str!("../job/splitter.py");

#[derive(Debug)]
pub struct TaskSplitter{
    total_tasks: u64,
    n: usize,
    iter: Py<PyIterator>
}

impl TaskSplitter{

    pub fn total_tasks(&self) -> u128{
        self.total_tasks as u128
    }
    pub fn new(data: Vec<u8>) -> Result<Self, anyhow::Error> {
        let (total_tasks, iter) = Python::with_gil(|py| -> PyResult<_>{
            let module = PyModule::from_code_bound(py, SPLITTER, "splitter.py", "splitter")?;
            let splitter = module.getattr("splitter")?;
            let result = splitter.call((PyBytes::new(py,&data),), None)?.extract::<(u64, Py<PyIterator>)>()?;
            Ok(result)
        })?;

        Ok(Self{ total_tasks, n: 0, iter })
    }

    pub fn iter<'py>(&'py mut self, py: Python<'py>) -> impl Iterator<Item = Result<(usize,Vec<u8>), PyErr>> + 'py{
        iter::from_fn(move || {
            self.iter
                .as_ref(py)
                .next()
                .map(|x| x.and_then(|x| x.extract::<Vec<u8>>()))
                .map(|x| x.map(|x| {
                    let res = (self.n, x);
                    self.n +=1;
                    res
                }))
        })
    }
}



