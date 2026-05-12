// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use pyo3::prelude::*;
use squick_core::{ScanOptions, Scanner};
use std::path::PathBuf;

#[pyfunction]
fn scan(root: &str) -> PyResult<String> {
    let mut scanner = Scanner::new(ScanOptions::default());
    let project = scanner
        .scan_project(&PathBuf::from(root))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
    Ok(squick_format::format_markdown(&project))
}

#[pymodule]
fn squick(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(scan, m)?)?;
    Ok(())
}
