//! HdExtComputationSchema - Ext computation (GPU/CPU) schema.
//!
//! Port of pxr/imaging/hd/extComputationSchema.h
//!
//! Defines input values, input computations (from other ext comps),
//! outputs, GLSL kernel, CPU callback, dispatch/element count.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Data source for `usize` (element count, dispatch count).
pub type HdSizetDataSource = dyn HdTypedSampledDataSource<usize>;
/// Handle for HdSizetDataSource.
pub type HdSizetDataSourceHandle = Arc<HdSizetDataSource>;

/// Data source for string (GLSL kernel source).
pub type HdStringDataSource = dyn HdTypedSampledDataSource<String>;
/// Handle for HdStringDataSource.
pub type HdStringDataSourceHandle = Arc<HdStringDataSource>;

static EXT_COMPUTATION: Lazy<Token> = Lazy::new(|| Token::new("extComputation"));
static INPUT_VALUES: Lazy<Token> = Lazy::new(|| Token::new("inputValues"));
static INPUT_COMPUTATIONS: Lazy<Token> = Lazy::new(|| Token::new("inputComputations"));
static OUTPUTS: Lazy<Token> = Lazy::new(|| Token::new("outputs"));
static GLSL_KERNEL: Lazy<Token> = Lazy::new(|| Token::new("glslKernel"));
static CPU_CALLBACK: Lazy<Token> = Lazy::new(|| Token::new("cpuCallback"));
static DISPATCH_COUNT: Lazy<Token> = Lazy::new(|| Token::new("dispatchCount"));
static ELEMENT_COUNT: Lazy<Token> = Lazy::new(|| Token::new("elementCount"));

/// Ext computation schema - GPU/CPU compute prim.
///
/// Fields: inputValues, inputComputations, outputs, glslKernel,
/// cpuCallback, dispatchCount, elementCount
#[derive(Debug, Clone)]
pub struct HdExtComputationSchema {
    schema: HdSchema,
}

impl HdExtComputationSchema {
    /// Creates schema from an extComputation container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Gets extComputation schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&EXT_COMPUTATION) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns true if the schema has valid data.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns the underlying extComputation container.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Gets the inputValues container.
    pub fn get_input_values(&self) -> Option<HdContainerDataSourceHandle> {
        let container = self.schema.get_container()?;
        let child = container.get(&INPUT_VALUES)?;
        cast_to_container(&child)
    }

    /// Gets the inputComputations container.
    pub fn get_input_computations(&self) -> Option<HdContainerDataSourceHandle> {
        let container = self.schema.get_container()?;
        let child = container.get(&INPUT_COMPUTATIONS)?;
        cast_to_container(&child)
    }

    /// Gets the outputs container.
    pub fn get_outputs(&self) -> Option<HdContainerDataSourceHandle> {
        let container = self.schema.get_container()?;
        let child = container.get(&OUTPUTS)?;
        cast_to_container(&child)
    }

    /// Gets the GLSL kernel source.
    pub fn get_glsl_kernel(&self) -> Option<HdStringDataSourceHandle> {
        self.schema.get_typed(&GLSL_KERNEL)
    }

    /// Gets the dispatch count (GPU workgroups).
    pub fn get_dispatch_count(&self) -> Option<HdSizetDataSourceHandle> {
        self.schema.get_typed(&DISPATCH_COUNT)
    }

    /// Gets the element count (CPU invocations).
    pub fn get_element_count(&self) -> Option<HdSizetDataSourceHandle> {
        self.schema.get_typed(&ELEMENT_COUNT)
    }

    /// Gets the CPU callback data source.
    ///
    /// Returns the data source for the cpuCallback field. Call GetValue(0.0)
    /// and extract HdExtComputationCpuCallbackValue to obtain the callback.
    pub fn get_cpu_callback(&self) -> Option<HdDataSourceBaseHandle> {
        self.schema
            .get_container()
            .and_then(|c| c.get(&CPU_CALLBACK))
    }

    /// Returns the extComputation schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &EXT_COMPUTATION
    }

    /// Returns the default locator for extComputation.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[EXT_COMPUTATION.clone()])
    }

    /// Returns the locator for inputValues (extComputation/inputValues).
    pub fn get_input_values_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[EXT_COMPUTATION.clone(), INPUT_VALUES.clone()])
    }

    /// Returns the locator for inputComputations (extComputation/inputComputations).
    pub fn get_input_computations_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[EXT_COMPUTATION.clone(), INPUT_COMPUTATIONS.clone()])
    }

    /// Returns the locator for outputs (extComputation/outputs).
    pub fn get_outputs_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[EXT_COMPUTATION.clone(), OUTPUTS.clone()])
    }

    /// Builds a retained container with all extComputation fields.
    pub fn build_retained(
        input_values: Option<HdContainerDataSourceHandle>,
        input_computations: Option<HdContainerDataSourceHandle>,
        outputs: Option<HdContainerDataSourceHandle>,
        glsl_kernel: Option<HdStringDataSourceHandle>,
        cpu_callback: Option<HdDataSourceBaseHandle>,
        dispatch_count: Option<HdSizetDataSourceHandle>,
        element_count: Option<HdSizetDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(v) = input_values {
            entries.push((INPUT_VALUES.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = input_computations {
            entries.push((INPUT_COMPUTATIONS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = outputs {
            entries.push((OUTPUTS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = glsl_kernel {
            entries.push((GLSL_KERNEL.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = cpu_callback {
            entries.push((CPU_CALLBACK.clone(), v));
        }
        if let Some(v) = dispatch_count {
            entries.push((DISPATCH_COUNT.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = element_count {
            entries.push((ELEMENT_COUNT.clone(), v as HdDataSourceBaseHandle));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdExtComputationSchema.
pub struct HdExtComputationSchemaBuilder {
    input_values: Option<HdContainerDataSourceHandle>,
    input_computations: Option<HdContainerDataSourceHandle>,
    outputs: Option<HdContainerDataSourceHandle>,
    glsl_kernel: Option<HdStringDataSourceHandle>,
    cpu_callback: Option<HdDataSourceBaseHandle>,
    dispatch_count: Option<HdSizetDataSourceHandle>,
    element_count: Option<HdSizetDataSourceHandle>,
}

impl HdExtComputationSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            input_values: None,
            input_computations: None,
            outputs: None,
            glsl_kernel: None,
            cpu_callback: None,
            dispatch_count: None,
            element_count: None,
        }
    }

    /// Sets the inputValues container.
    pub fn set_input_values(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.input_values = Some(v);
        self
    }

    /// Sets the inputComputations container.
    pub fn set_input_computations(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.input_computations = Some(v);
        self
    }

    /// Sets the outputs container.
    pub fn set_outputs(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.outputs = Some(v);
        self
    }

    /// Sets the GLSL kernel source.
    pub fn set_glsl_kernel(mut self, v: HdStringDataSourceHandle) -> Self {
        self.glsl_kernel = Some(v);
        self
    }

    /// Sets the CPU callback handle.
    pub fn set_cpu_callback(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.cpu_callback = Some(v);
        self
    }

    /// Sets the dispatch count.
    pub fn set_dispatch_count(mut self, v: HdSizetDataSourceHandle) -> Self {
        self.dispatch_count = Some(v);
        self
    }

    /// Sets the element count.
    pub fn set_element_count(mut self, v: HdSizetDataSourceHandle) -> Self {
        self.element_count = Some(v);
        self
    }

    /// Builds the container with all set fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdExtComputationSchema::build_retained(
            self.input_values,
            self.input_computations,
            self.outputs,
            self.glsl_kernel,
            self.cpu_callback,
            self.dispatch_count,
            self.element_count,
        )
    }
}
