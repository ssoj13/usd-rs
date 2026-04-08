//! pxr.Ts — Time Spline Python bindings.
//!
//! Drop-in replacement for `pxr.Ts` from C++ OpenUSD.
//! Wraps usd_ts Spline, Knot, and related types.

use pyo3::prelude::*;

use usd_ts::knot::Knot;
use usd_ts::spline::Spline;
use usd_ts::types::{InterpMode, TangentAlgorithm};

// ============================================================================
// TsSpline
// ============================================================================

/// A time-value spline with knots.
///
/// Matches C++ `TsSpline`.
#[pyclass(skip_from_py_object, name = "Spline", module = "pxr_rs.Ts")]
#[derive(Clone)]
pub struct PySpline {
    pub(crate) inner: Spline,
}

#[pymethods]
impl PySpline {
    #[new]
    fn new() -> Self {
        Self {
            inner: Spline::new(),
        }
    }

    /// Set a knot on the spline.
    ///
    /// Matches C++ `TsSpline::SetKnot()`.
    #[allow(non_snake_case)]
    fn SetKnot(&mut self, knot: &PyKnot) {
        self.inner.set_knot(knot.inner.clone());
    }

    fn __repr__(&self) -> String {
        "Ts.Spline()".to_string()
    }
}

// ============================================================================
// TsKnot
// ============================================================================

/// A single knot in a spline, holding time + value + tangent info.
///
/// Matches C++ `TsKnot`.
#[pyclass(skip_from_py_object, name = "Knot", module = "pxr_rs.Ts")]
#[derive(Clone)]
pub struct PyKnot {
    pub(crate) inner: Knot,
}

#[pymethods]
impl PyKnot {
    /// Create a knot with optional keyword arguments.
    ///
    /// Matches C++ `TsKnot` constructor with named parameters:
    ///   time, value, nextInterp, postTanAlgorithm
    #[new]
    #[pyo3(signature = (time = 0.0, value = 0.0, nextInterp = None, postTanAlgorithm = None))]
    fn new(
        time: f64,
        value: f64,
        #[allow(non_snake_case)] nextInterp: Option<&PyInterpMode>,
        #[allow(non_snake_case)] postTanAlgorithm: Option<&PyTangentAlgorithm>,
    ) -> Self {
        let mut knot = Knot::new();
        knot.set_time(time);
        knot.set_value(value);
        if let Some(interp) = nextInterp {
            knot.set_interp_mode(interp.inner);
        }
        if let Some(algo) = postTanAlgorithm {
            knot.set_post_tan_algorithm(algo.inner);
        }
        Self { inner: knot }
    }

    fn __repr__(&self) -> String {
        format!(
            "Ts.Knot(time={}, value={})",
            self.inner.time(),
            self.inner.value()
        )
    }
}

// ============================================================================
// InterpMode enum
// ============================================================================

/// Interpolation mode for a spline segment.
///
/// Matches C++ `TsInterpMode` values.
#[pyclass(skip_from_py_object, name = "InterpMode", module = "pxr_rs.Ts")]
#[derive(Clone)]
pub struct PyInterpMode {
    inner: InterpMode,
}

#[pymethods]
impl PyInterpMode {
    fn __repr__(&self) -> String {
        format!("Ts.InterpMode({})", self.inner)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

// ============================================================================
// TangentAlgorithm enum
// ============================================================================

/// Algorithm for computing tangent values.
///
/// Matches C++ `TsTangentAlgorithm` values.
#[pyclass(skip_from_py_object, name = "TangentAlgorithm", module = "pxr_rs.Ts")]
#[derive(Clone)]
pub struct PyTangentAlgorithm {
    inner: TangentAlgorithm,
}

#[pymethods]
impl PyTangentAlgorithm {
    fn __repr__(&self) -> String {
        format!("Ts.TangentAlgorithm({})", self.inner)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

// ============================================================================
// Module registration
// ============================================================================

/// Register all Ts classes and constants into the `pxr.Ts` submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySpline>()?;
    m.add_class::<PyKnot>()?;
    m.add_class::<PyInterpMode>()?;
    m.add_class::<PyTangentAlgorithm>()?;

    // Module-level interp mode constants matching C++ Ts.InterpHeld etc.
    m.add(
        "InterpHeld",
        PyInterpMode {
            inner: InterpMode::Held,
        },
    )?;
    m.add(
        "InterpLinear",
        PyInterpMode {
            inner: InterpMode::Linear,
        },
    )?;
    m.add(
        "InterpCurve",
        PyInterpMode {
            inner: InterpMode::Curve,
        },
    )?;
    m.add(
        "InterpValueBlock",
        PyInterpMode {
            inner: InterpMode::ValueBlock,
        },
    )?;

    // Module-level tangent algorithm constants matching C++ Ts.TangentAlgorithmNone etc.
    m.add(
        "TangentAlgorithmNone",
        PyTangentAlgorithm {
            inner: TangentAlgorithm::None,
        },
    )?;
    m.add(
        "TangentAlgorithmCustom",
        PyTangentAlgorithm {
            inner: TangentAlgorithm::Custom,
        },
    )?;
    m.add(
        "TangentAlgorithmAutoEase",
        PyTangentAlgorithm {
            inner: TangentAlgorithm::AutoEase,
        },
    )?;

    Ok(())
}
