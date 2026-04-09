//! OpenUSD `pxr/usd/usdUtils/constantsGroup.py` — logic parity in Rust (PyO3), no `.py` in wheel.
//!
//! Metaclass is built with `types.new_class(..., (type,), exec_body)` because PyO3 0.28
//! does not allow `#[pyclass(extends = PyType)]`.
//!
//! CPython's `_PyObject_LookupSpecial` calls `__contains__` / `__iter__` / etc. with a single
//! user argument (`PyObject_CallOneArg`); the callable must be a descriptor whose `__get__`
//! returns `types.MethodType(inner, owner)` (see `Objects/abstract.c`).

use pyo3::exceptions::{PyAttributeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyList, PyTuple};

/// Descriptor so `_PyObject_LookupSpecial` + `PyObject_CallOneArg` work like a Python `def` on the metaclass.
#[pyclass(name = "_DunderBindingDescr", module = "pxr.UsdUtils.constantsGroup")]
struct PyDunderBindingDescr {
    inner: Py<PyAny>,
}

#[pymethods]
impl PyDunderBindingDescr {
    fn __get__(
        &self,
        py: Python<'_>,
        instance: Option<Py<PyAny>>,
        owner: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        // `_PyObject_LookupSpecial` may call `descr.__get__` with (instance, owner) such that
        // `owner` is the metaclass `M` instead of the constant-group class `Test`; prefer the
        // object that actually carries `_all` (see Pixar `constantsGroup.py`).
        let cls = pick_constant_group_class(py, instance, owner)?;
        let types = py.import("types")?;
        let mt = types.getattr("MethodType")?;
        mt.call((self.inner.clone_ref(py), cls), None)
            .map(|b| b.unbind())
    }
}

fn pick_constant_group_class<'py>(
    py: Python<'py>,
    instance: Option<Py<PyAny>>,
    owner: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    if let Some(p) = instance {
        let b = p.bind(py);
        if b.hasattr("_all").unwrap_or(false) {
            return Ok(b.clone());
        }
    }
    if let Some(o) = owner {
        if o.hasattr("_all").unwrap_or(false) {
            return Ok(o.clone());
        }
    }
    Err(PyTypeError::new_err(
        "DunderBindingDescr.__get__ could not resolve a class with _all",
    ))
}

#[pyfunction]
#[pyo3(name = "meta_constants_group_new", signature = (mcs, name, bases, namespace, **kwargs))]
fn meta_constants_group_new(
    mcs: &Bound<'_, PyAny>,
    name: &str,
    bases: &Bound<'_, PyTuple>,
    namespace: &Bound<'_, PyDict>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let _ = kwargs;
    let py = mcs.py();
    let builtins = py.import("builtins")?;
    let ty = builtins.getattr("type")?;
    let types_mod = py.import("types")?;
    let function_type = types_mod.getattr("FunctionType")?;
    let isinstance = builtins.getattr("isinstance")?;
    let staticmethod_ctor = builtins.getattr("staticmethod")?;

    if name == "ConstantsGroup" {
        let out = ty.call_method("__new__", (mcs, name, bases, namespace), None)?;
        return Ok(out.unbind());
    }

    let items = namespace.call_method0("items")?;
    let list: Bound<'_, PyAny> = builtins.getattr("list")?.call1((items,))?;
    let list: Bound<'_, PyList> = list.cast_into()?;

    let mut all_constants: Vec<Py<PyAny>> = Vec::new();

    for i in 0..list.len() {
        let pair = list.get_item(i)?;
        let key: String = pair.get_item(0)?.extract()?;
        if key.starts_with('_') {
            continue;
        }
        let val = pair.get_item(1)?;

        let classmethod_type = builtins.getattr("classmethod")?;
        let staticmethod_type = builtins.getattr("staticmethod")?;
        let is_cm: bool = isinstance.call1((&val, &classmethod_type))?.extract()?;
        let is_sm: bool = isinstance.call1((&val, &staticmethod_type))?.extract()?;
        if is_cm || is_sm {
            continue;
        }

        let is_fn: bool = isinstance.call1((&val, &function_type))?.extract()?;
        all_constants.push(val.clone().unbind());
        if is_fn {
            let sm = staticmethod_ctor.call1((&val,))?;
            namespace.set_item(&key, sm)?;
        }
    }

    let tup = PyTuple::new(py, all_constants)?;
    namespace.set_item("_all", tup)?;

    let out = ty.call_method("__new__", (mcs, name, bases, namespace), None)?;
    Ok(out.unbind())
}

#[pyfunction]
#[pyo3(name = "meta_constants_group_setattr")]
fn meta_constants_group_setattr(
    _cls: &Bound<'_, PyAny>,
    _name: &str,
    _value: &Bound<'_, PyAny>,
) -> PyResult<()> {
    Err(PyAttributeError::new_err(
        "Constant groups cannot be modified.",
    ))
}

#[pyfunction]
#[pyo3(name = "meta_constants_group_delattr")]
fn meta_constants_group_delattr(_cls: &Bound<'_, PyAny>, _name: &str) -> PyResult<()> {
    Err(PyAttributeError::new_err(
        "Constant groups cannot be modified.",
    ))
}

#[pyfunction]
#[pyo3(name = "meta_constants_group_len")]
fn meta_constants_group_len(cls: &Bound<'_, PyAny>) -> PyResult<usize> {
    let py = cls.py();
    let _all = cls.getattr("_all")?;
    let builtins = py.import("builtins")?;
    let n: usize = builtins.getattr("len")?.call1((_all,))?.extract()?;
    Ok(n)
}

#[pyfunction]
#[pyo3(name = "meta_constants_group_contains")]
fn meta_constants_group_contains(
    cls: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let _all = cls.getattr("_all")?;
    Ok(_all.contains(value)?)
}

#[pyfunction]
#[pyo3(name = "meta_constants_group_iter")]
fn meta_constants_group_iter(cls: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let _all = cls.getattr("_all")?;
    _all.call_method0("__iter__").map(|i| i.unbind())
}

#[pyfunction]
#[pyo3(name = "constants_group_instance_new", signature = (cls, *args, **kwargs))]
fn constants_group_instance_new(
    cls: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let _ = (cls, args, kwargs);
    Err(PyTypeError::new_err(
        "ConstantsGroup objects cannot be created.",
    ))
}

/// Registers `_MetaConstantsGroup` and `ConstantsGroup` on `constantsGroup` submodule.
pub fn register_constants_group(py: Python<'_>, cg_mod: &Bound<'_, PyModule>) -> PyResult<()> {
    cg_mod.add_class::<PyDunderBindingDescr>()?;

    let builtins = py.import("builtins")?;
    let types_mod = py.import("types")?;
    let new_class = types_mod.getattr("new_class")?;
    let type_obj = builtins.getattr("type")?;
    let object = builtins.getattr("object")?;

    let f_meta_new: Py<PyAny> = wrap_pyfunction!(meta_constants_group_new, cg_mod)?
        .into_any()
        .unbind();
    let f_setattr: Py<PyAny> = wrap_pyfunction!(meta_constants_group_setattr, cg_mod)?
        .into_any()
        .unbind();
    let f_delattr: Py<PyAny> = wrap_pyfunction!(meta_constants_group_delattr, cg_mod)?
        .into_any()
        .unbind();
    let f_len: Py<PyAny> = wrap_pyfunction!(meta_constants_group_len, cg_mod)?
        .into_any()
        .unbind();
    let f_contains: Py<PyAny> = wrap_pyfunction!(meta_constants_group_contains, cg_mod)?
        .into_any()
        .unbind();
    let f_iter: Py<PyAny> = wrap_pyfunction!(meta_constants_group_iter, cg_mod)?
        .into_any()
        .unbind();

    let exec_meta = PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>, _kw: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
            let py = args.py();
            let ns_any = args.get_item(0)?;
            let ns = ns_any.cast::<PyDict>()?;
            ns.set_item("__new__", f_meta_new.clone_ref(py))?;
            let d_setattr = Py::new(
                py,
                PyDunderBindingDescr {
                    inner: f_setattr.clone_ref(py),
                },
            )?;
            let d_delattr = Py::new(
                py,
                PyDunderBindingDescr {
                    inner: f_delattr.clone_ref(py),
                },
            )?;
            let d_len = Py::new(
                py,
                PyDunderBindingDescr {
                    inner: f_len.clone_ref(py),
                },
            )?;
            let d_contains = Py::new(
                py,
                PyDunderBindingDescr {
                    inner: f_contains.clone_ref(py),
                },
            )?;
            let d_iter = Py::new(
                py,
                PyDunderBindingDescr {
                    inner: f_iter.clone_ref(py),
                },
            )?;
            ns.set_item("__setattr__", d_setattr)?;
            ns.set_item("__delattr__", d_delattr)?;
            ns.set_item("__len__", d_len)?;
            ns.set_item("__contains__", d_contains)?;
            ns.set_item("__iter__", d_iter)?;
            Ok(py.None().into_any())
        },
    )?;

    let bases_meta = PyTuple::new(py, [type_obj])?;
    let kwds_none = PyDict::new(py);
    let meta = new_class.call(
        ("_MetaConstantsGroup", bases_meta, kwds_none, exec_meta),
        None,
    )?;

    let f_cg_new: Py<PyAny> = wrap_pyfunction!(constants_group_instance_new, cg_mod)?
        .into_any()
        .unbind();
    let exec_cg = PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>, _kw: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
            let py = args.py();
            let ns_any = args.get_item(0)?;
            let ns = ns_any.cast::<PyDict>()?;
            ns.set_item(
                "__doc__",
                "The base constant group class, intended to be inherited by actual groups\n    of constants.\n    ",
            )?;
            ns.set_item("__new__", f_cg_new.clone_ref(py))?;
            Ok(py.None().into_any())
        },
    )?;

    let bases_cg = PyTuple::new(py, [object])?;
    let kwds = PyDict::new(py);
    kwds.set_item("metaclass", &meta)?;
    let cg = new_class.call(("ConstantsGroup", bases_cg, kwds, exec_cg), None)?;

    cg_mod.setattr("_MetaConstantsGroup", meta)?;
    cg_mod.setattr("ConstantsGroup", cg)?;
    Ok(())
}
