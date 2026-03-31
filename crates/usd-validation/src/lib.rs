//! USD Validation Framework
//!
//! Complete port of the C++ UsdValidation module. Provides a registry of
//! validators that can check layers, stages, and prims for correctness.
//! Validators produce [`ValidationError`] values describing any issues found.
//!
//! The framework consists of:
//! - [`ValidationError`] / [`ErrorSite`] / [`ErrorType`] for reporting problems
//! - [`Validator`] / [`ValidatorSuite`] for encapsulating checks
//! - [`ValidationRegistry`] singleton for registering and looking up validators
//! - [`ValidationContext`] for running a set of validators in parallel
//! - [`ValidationFixer`] for programmatic error repair
//! - Three built-in core validators

pub mod geom_validators;
pub mod physics_validators;
pub mod shade_validators;
pub mod utils_validators;

use usd_core::edit_target::EditTarget;
use usd_core::prim::Prim;
use usd_core::stage::Stage;
use usd_core::time_code::TimeCode;
use usd_gf::Interval;
use usd_sdf::{Layer, Path};
use usd_tf::Token;
use usd_vt::Value;

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

// ============================================================================
// ErrorType
// ============================================================================

/// Severity level for a validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorType {
    /// Sentinel: no error.
    None,
    /// A real error that should be fixed.
    Error,
    /// A warning worth investigating.
    Warn,
    /// Informational note.
    Info,
}

impl std::fmt::Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Error => write!(f, "Error"),
            Self::Warn => write!(f, "Warn"),
            Self::Info => write!(f, "Info"),
        }
    }
}

// ============================================================================
// ErrorSite
// ============================================================================

/// Identifies the location where a validation error was found.
///
/// A site may reference a stage (with optional layer) and a path to the
/// offending prim or property. Alternatively it may reference only a layer.
#[derive(Clone)]
pub struct ErrorSite {
    /// Stage context (if any).
    stage: Option<Arc<Stage>>,
    /// Layer context (if any).
    layer: Option<Arc<Layer>>,
    /// Path to the prim or property with the issue.
    object_path: Path,
}

impl std::fmt::Debug for ErrorSite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorSite")
            .field("has_stage", &self.stage.is_some())
            .field("has_layer", &self.layer.is_some())
            .field("object_path", &self.object_path)
            .finish()
    }
}

impl ErrorSite {
    /// Build a site referencing a layer and a path within it.
    pub fn from_layer(layer: &Arc<Layer>, path: Path) -> Self {
        Self {
            stage: None,
            layer: Some(Arc::clone(layer)),
            object_path: path,
        }
    }

    /// Build a site referencing a stage, a path, and optionally a layer.
    pub fn from_stage(stage: &Arc<Stage>, path: Path, layer: Option<&Arc<Layer>>) -> Self {
        Self {
            stage: Some(Arc::clone(stage)),
            layer: layer.map(Arc::clone),
            object_path: path,
        }
    }

    /// An empty / invalid site.
    pub fn empty() -> Self {
        Self {
            stage: None,
            layer: None,
            object_path: Path::empty(),
        }
    }

    /// True if the site has at least a layer or a stage.
    pub fn is_valid(&self) -> bool {
        self.stage.is_some() || self.layer.is_some()
    }

    /// True if `object_path` points at a prim.
    pub fn is_prim(&self) -> bool {
        self.object_path.is_prim_path()
    }

    /// True if `object_path` points at a property.
    pub fn is_property(&self) -> bool {
        self.object_path.is_property_path()
    }

    /// Layer associated with this site.
    pub fn get_layer(&self) -> Option<&Arc<Layer>> {
        self.layer.as_ref()
    }

    /// Stage associated with this site.
    pub fn get_stage(&self) -> Option<&Arc<Stage>> {
        self.stage.as_ref()
    }

    /// Object path within the site.
    pub fn get_path(&self) -> &Path {
        &self.object_path
    }

    /// Resolve the prim referenced by this site, if possible.
    pub fn get_prim(&self) -> Option<Prim> {
        let stage = self.stage.as_ref()?;
        if self.object_path.is_prim_path() {
            stage.get_prim_at_path(&self.object_path)
        } else {
            None
        }
    }
}

impl PartialEq for ErrorSite {
    fn eq(&self, other: &Self) -> bool {
        // Compare by identity of stage/layer arcs and path equality.
        let same_stage = match (&self.stage, &other.stage) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        let same_layer = match (&self.layer, &other.layer) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        same_stage && same_layer && self.object_path == other.object_path
    }
}

impl Eq for ErrorSite {}

// ============================================================================
// ValidationError
// ============================================================================

/// A single validation issue produced by a [`Validator`].
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Short machine-readable name (e.g. "missingDefaultPrim").
    name: Token,
    /// Severity.
    error_type: ErrorType,
    /// Where the problem was found.
    sites: Vec<ErrorSite>,
    /// Human-readable description.
    message: String,
    /// Optional associated data payload.
    data: Option<Value>,
    /// Name of the validator that produced this error.
    validator_name: Token,
}

impl ValidationError {
    /// Create a new validation error with sites.
    pub fn new(name: Token, error_type: ErrorType, sites: Vec<ErrorSite>, message: String) -> Self {
        Self {
            name,
            error_type,
            sites,
            message,
            data: None,
            validator_name: Token::new(""),
        }
    }

    /// Create a simple error without sites.
    pub fn simple(name: Token, error_type: ErrorType, message: String) -> Self {
        Self::new(name, error_type, Vec::new(), message)
    }

    /// Builder: attach a data payload.
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Builder: set the originating validator name.
    pub fn with_validator(mut self, validator: Token) -> Self {
        self.validator_name = validator;
        self
    }

    // -- Accessors --

    /// Error name token.
    pub fn get_name(&self) -> &Token {
        &self.name
    }

    /// Error severity.
    pub fn get_type(&self) -> ErrorType {
        self.error_type
    }

    /// All sites where this error was found.
    pub fn get_sites(&self) -> &[ErrorSite] {
        &self.sites
    }

    /// Human-readable error message.
    pub fn get_message(&self) -> &str {
        &self.message
    }

    /// Optional data payload.
    pub fn get_data(&self) -> Option<&Value> {
        self.data.as_ref()
    }

    /// Name of the validator that produced this error.
    pub fn get_validator_name(&self) -> &Token {
        &self.validator_name
    }

    /// Formatted identifier: "validatorName: errorName".
    pub fn get_identifier(&self) -> String {
        if self.validator_name.as_str().is_empty() {
            self.name.as_str().to_string()
        } else {
            format!("{}: {}", self.validator_name.as_str(), self.name.as_str())
        }
    }

    /// Human-readable one-liner: "[ERROR] identifier -- message".
    pub fn get_error_as_string(&self) -> String {
        format!(
            "[{}] {} -- {}",
            self.error_type,
            self.get_identifier(),
            self.message
        )
    }

    /// True when `error_type` is [`ErrorType::None`].
    pub fn has_no_error(&self) -> bool {
        self.error_type == ErrorType::None
    }
}

impl PartialEq for ValidationError {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.error_type == other.error_type
            && self.sites == other.sites
            && self.message == other.message
    }
}

impl Eq for ValidationError {}

// ============================================================================
// ValidationTimeRange
// ============================================================================

/// Describes the time domain over which validation should be performed.
#[derive(Debug, Clone)]
pub struct ValidationTimeRange {
    /// Interval of frame numbers to check.
    interval: Interval,
    /// Whether to also evaluate at the default time code.
    include_time_code_default: bool,
}

impl Default for ValidationTimeRange {
    /// Full interval with default time-code included.
    fn default() -> Self {
        Self {
            interval: Interval::full(),
            include_time_code_default: true,
        }
    }
}

impl ValidationTimeRange {
    /// Construct from a single time code (point interval).
    pub fn from_time_code(tc: &TimeCode) -> Self {
        if tc.is_default() {
            Self {
                interval: Interval::full(),
                include_time_code_default: true,
            }
        } else {
            let v = tc.value();
            Self {
                interval: Interval::from_point(v),
                include_time_code_default: false,
            }
        }
    }

    /// Construct from an explicit interval and default flag.
    pub fn from_interval(interval: Interval, include_default: bool) -> Self {
        Self {
            interval,
            include_time_code_default: include_default,
        }
    }

    /// Whether the default time code is included.
    pub fn includes_time_code_default(&self) -> bool {
        self.include_time_code_default
    }

    /// The time interval.
    pub fn get_interval(&self) -> &Interval {
        &self.interval
    }
}

// ============================================================================
// ValidationFixer
// ============================================================================

/// Function type for fixer operations: given an error, an edit target, and a
/// time code, attempt to fix the error and return success.
pub type FixerFn = Arc<dyn Fn(&ValidationError, &EditTarget, &TimeCode) -> bool + Send + Sync>;

/// A programmatic repair action associated with a specific error.
#[derive(Clone)]
pub struct ValidationFixer {
    name: Token,
    description: String,
    error_name: Token,
    keywords: Vec<Token>,
    apply_fn: FixerFn,
    can_apply_fn: FixerFn,
}

impl std::fmt::Debug for ValidationFixer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidationFixer")
            .field("name", &self.name)
            .field("error_name", &self.error_name)
            .finish()
    }
}

impl ValidationFixer {
    /// Create a new fixer with apply and can-apply callbacks.
    pub fn new(
        name: Token,
        description: String,
        error_name: Token,
        keywords: Vec<Token>,
        apply_fn: FixerFn,
        can_apply_fn: FixerFn,
    ) -> Self {
        Self {
            name,
            description,
            error_name,
            keywords,
            apply_fn,
            can_apply_fn,
        }
    }

    /// Fixer name token.
    pub fn get_name(&self) -> &Token {
        &self.name
    }

    /// Human-readable description.
    pub fn get_description(&self) -> &str {
        &self.description
    }

    /// Name of the error this fixer addresses.
    pub fn get_error_name(&self) -> &Token {
        &self.error_name
    }

    /// True if this fixer targets the given error name.
    pub fn is_associated_with_error_name(&self, name: &Token) -> bool {
        self.error_name == *name
    }

    /// Keywords for searching / filtering fixers.
    pub fn get_keywords(&self) -> &[Token] {
        &self.keywords
    }

    /// True if the fixer has a given keyword.
    pub fn has_keyword(&self, kw: &Token) -> bool {
        self.keywords.contains(kw)
    }

    /// Test if the fixer can be applied to the given error.
    pub fn can_apply_fix(
        &self,
        error: &ValidationError,
        target: &EditTarget,
        tc: &TimeCode,
    ) -> bool {
        (self.can_apply_fn)(error, target, tc)
    }

    /// Apply the fix. Returns true on success.
    pub fn apply_fix(&self, error: &ValidationError, target: &EditTarget, tc: &TimeCode) -> bool {
        (self.apply_fn)(error, target, tc)
    }
}

// ============================================================================
// ValidatorMetadata
// ============================================================================

/// Descriptive metadata for a registered validator.
#[derive(Debug, Clone)]
pub struct ValidatorMetadata {
    /// Unique fully-qualified name (e.g. "usdValidation:CompositionErrorTest").
    pub name: Token,
    /// Search keywords.
    pub keywords: Vec<Token>,
    /// Documentation string.
    pub doc: String,
    /// Schema types this validator applies to.
    pub schema_types: Vec<Token>,
    /// Whether the validator needs time-sampled evaluation.
    pub is_time_dependent: bool,
    /// Whether this describes a suite rather than a single validator.
    pub is_suite: bool,
}

impl ValidatorMetadata {
    /// Create metadata with default values from a name token.
    pub fn new(name: Token) -> Self {
        Self {
            name,
            keywords: Vec::new(),
            doc: String::new(),
            schema_types: Vec::new(),
            is_time_dependent: false,
            is_suite: false,
        }
    }

    /// Builder: set keywords.
    pub fn with_keywords(mut self, kw: Vec<Token>) -> Self {
        self.keywords = kw;
        self
    }

    /// Builder: set doc.
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }

    /// Builder: set schema types.
    pub fn with_schema_types(mut self, types: Vec<Token>) -> Self {
        self.schema_types = types;
        self
    }

    /// Builder: set time dependent.
    pub fn with_time_dependent(mut self, td: bool) -> Self {
        self.is_time_dependent = td;
        self
    }

    /// Builder: mark as suite.
    pub fn as_suite(mut self) -> Self {
        self.is_suite = true;
        self
    }
}

// ============================================================================
// Task function types + ValidatorTask enum
// ============================================================================

/// Validate a layer (no time component).
pub type ValidateLayerTaskFn = Arc<dyn Fn(&Arc<Layer>) -> Vec<ValidationError> + Send + Sync>;

/// Validate a stage over a time range.
pub type ValidateStageTaskFn =
    Arc<dyn Fn(&Arc<Stage>, &ValidationTimeRange) -> Vec<ValidationError> + Send + Sync>;

/// Validate a single prim over a time range.
pub type ValidatePrimTaskFn =
    Arc<dyn Fn(&Prim, &ValidationTimeRange) -> Vec<ValidationError> + Send + Sync>;

/// Discriminated union of validator task callables.
pub enum ValidatorTask {
    /// Layer validation closure.
    Layer(ValidateLayerTaskFn),
    /// Stage validation closure.
    Stage(ValidateStageTaskFn),
    /// Prim validation closure.
    Prim(ValidatePrimTaskFn),
    /// No task assigned.
    None,
}

impl std::fmt::Debug for ValidatorTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Layer(_) => write!(f, "ValidatorTask::Layer(..)"),
            Self::Stage(_) => write!(f, "ValidatorTask::Stage(..)"),
            Self::Prim(_) => write!(f, "ValidatorTask::Prim(..)"),
            Self::None => write!(f, "ValidatorTask::None"),
        }
    }
}

// ============================================================================
// Validator
// ============================================================================

/// A single validation check. Not clonable because it owns closures.
pub struct Validator {
    metadata: ValidatorMetadata,
    task: ValidatorTask,
    fixers: Vec<ValidationFixer>,
}

impl std::fmt::Debug for Validator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Validator")
            .field("metadata", &self.metadata)
            .field("task", &self.task)
            .field("fixers_count", &self.fixers.len())
            .finish()
    }
}

impl Validator {
    /// Construct a new validator.
    pub fn new(
        metadata: ValidatorMetadata,
        task: ValidatorTask,
        fixers: Vec<ValidationFixer>,
    ) -> Self {
        Self {
            metadata,
            task,
            fixers,
        }
    }

    /// Validator metadata (name, keywords, doc, etc.).
    pub fn get_metadata(&self) -> &ValidatorMetadata {
        &self.metadata
    }

    /// All fixers registered with this validator.
    pub fn get_fixers(&self) -> &[ValidationFixer] {
        &self.fixers
    }

    /// Find a fixer by name.
    pub fn get_fixer_by_name(&self, name: &Token) -> Option<&ValidationFixer> {
        self.fixers.iter().find(|f| f.get_name() == name)
    }

    /// Run a layer validation. Panics if the task is not Layer.
    pub fn validate_layer(&self, layer: &Arc<Layer>) -> Vec<ValidationError> {
        match &self.task {
            ValidatorTask::Layer(f) => {
                let mut errs = f(layer);
                let vname = self.metadata.name.clone();
                for e in &mut errs {
                    e.validator_name = vname.clone();
                }
                errs
            }
            _ => Vec::new(),
        }
    }

    /// Run a stage validation.
    pub fn validate_stage(
        &self,
        stage: &Arc<Stage>,
        time_range: &ValidationTimeRange,
    ) -> Vec<ValidationError> {
        match &self.task {
            ValidatorTask::Stage(f) => {
                let mut errs = f(stage, time_range);
                let vname = self.metadata.name.clone();
                for e in &mut errs {
                    e.validator_name = vname.clone();
                }
                errs
            }
            _ => Vec::new(),
        }
    }

    /// Run a prim validation.
    pub fn validate_prim(
        &self,
        prim: &Prim,
        time_range: &ValidationTimeRange,
    ) -> Vec<ValidationError> {
        match &self.task {
            ValidatorTask::Prim(f) => {
                let mut errs = f(prim, time_range);
                let vname = self.metadata.name.clone();
                for e in &mut errs {
                    e.validator_name = vname.clone();
                }
                errs
            }
            _ => Vec::new(),
        }
    }

    /// True if the task variant is Layer.
    pub fn is_layer_validator(&self) -> bool {
        matches!(&self.task, ValidatorTask::Layer(_))
    }

    /// True if the task variant is Stage.
    pub fn is_stage_validator(&self) -> bool {
        matches!(&self.task, ValidatorTask::Stage(_))
    }

    /// True if the task variant is Prim.
    pub fn is_prim_validator(&self) -> bool {
        matches!(&self.task, ValidatorTask::Prim(_))
    }
}

// ============================================================================
// ValidatorSuite
// ============================================================================

/// A named group of validators.
pub struct ValidatorSuite {
    metadata: ValidatorMetadata,
    /// Names of the contained validators.
    contained_validators: Vec<Token>,
}

impl std::fmt::Debug for ValidatorSuite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidatorSuite")
            .field("metadata", &self.metadata)
            .field("contained", &self.contained_validators)
            .finish()
    }
}

impl ValidatorSuite {
    /// Create a suite from metadata and a list of validator names.
    pub fn new(metadata: ValidatorMetadata, contained_validators: Vec<Token>) -> Self {
        Self {
            metadata,
            contained_validators,
        }
    }

    /// Suite metadata.
    pub fn get_metadata(&self) -> &ValidatorMetadata {
        &self.metadata
    }

    /// Names of validators contained in this suite.
    pub fn get_contained_validators(&self) -> &[Token] {
        &self.contained_validators
    }
}

// ============================================================================
// ValidationRegistry  (thread-safe singleton)
// ============================================================================

/// Global registry of all known validators and suites.
///
/// Access via [`ValidationRegistry::get_instance()`].
pub struct ValidationRegistry {
    inner: RwLock<RegistryInner>,
}

struct RegistryInner {
    validators: HashMap<Token, Validator>,
    suites: HashMap<Token, ValidatorSuite>,
    /// Keyword -> set of validator names that have that keyword.
    keyword_index: HashMap<Token, Vec<Token>>,
    /// SchemaType -> set of validator names for that type.
    schema_type_index: HashMap<Token, Vec<Token>>,
}

impl RegistryInner {
    fn new() -> Self {
        Self {
            validators: HashMap::new(),
            suites: HashMap::new(),
            keyword_index: HashMap::new(),
            schema_type_index: HashMap::new(),
        }
    }

    /// Rebuild secondary indices for a single validator's metadata.
    fn index_metadata(&mut self, meta: &ValidatorMetadata) {
        for kw in &meta.keywords {
            self.keyword_index
                .entry(kw.clone())
                .or_default()
                .push(meta.name.clone());
        }
        for st in &meta.schema_types {
            self.schema_type_index
                .entry(st.clone())
                .or_default()
                .push(meta.name.clone());
        }
    }
}

static REGISTRY: OnceLock<ValidationRegistry> = OnceLock::new();

impl ValidationRegistry {
    /// Get the global singleton.
    pub fn get_instance() -> &'static Self {
        REGISTRY.get_or_init(|| {
            let reg = Self {
                inner: RwLock::new(RegistryInner::new()),
            };
            // Register built-in validators.
            reg.register_core_validators();
            reg
        })
    }

    // -- Registration --

    /// Register a layer validator.
    pub fn register_layer_validator(
        &self,
        metadata: ValidatorMetadata,
        task: ValidateLayerTaskFn,
        fixers: Vec<ValidationFixer>,
    ) {
        let mut inner = self.inner.write();
        inner.index_metadata(&metadata);
        let name = metadata.name.clone();
        let v = Validator::new(metadata, ValidatorTask::Layer(task), fixers);
        inner.validators.insert(name, v);
    }

    /// Register a stage validator.
    pub fn register_stage_validator(
        &self,
        metadata: ValidatorMetadata,
        task: ValidateStageTaskFn,
        fixers: Vec<ValidationFixer>,
    ) {
        let mut inner = self.inner.write();
        inner.index_metadata(&metadata);
        let name = metadata.name.clone();
        let v = Validator::new(metadata, ValidatorTask::Stage(task), fixers);
        inner.validators.insert(name, v);
    }

    /// Register a prim validator.
    pub fn register_prim_validator(
        &self,
        metadata: ValidatorMetadata,
        task: ValidatePrimTaskFn,
        fixers: Vec<ValidationFixer>,
    ) {
        let mut inner = self.inner.write();
        inner.index_metadata(&metadata);
        let name = metadata.name.clone();
        let v = Validator::new(metadata, ValidatorTask::Prim(task), fixers);
        inner.validators.insert(name, v);
    }

    /// Register a validator suite (group of validators by name).
    pub fn register_validator_suite(
        &self,
        metadata: ValidatorMetadata,
        validator_names: Vec<Token>,
    ) {
        let mut inner = self.inner.write();
        inner.index_metadata(&metadata);
        let name = metadata.name.clone();
        let suite = ValidatorSuite::new(metadata, validator_names);
        inner.suites.insert(name, suite);
    }

    // -- Queries --

    /// True if a validator with `name` is registered.
    pub fn has_validator(&self, name: &Token) -> bool {
        self.inner.read().validators.contains_key(name)
    }

    /// True if a suite with `name` is registered.
    pub fn has_validator_suite(&self, name: &Token) -> bool {
        self.inner.read().suites.contains_key(name)
    }

    /// Access a validator by name. Holds a read lock while the callback runs.
    pub fn with_validator<R>(&self, name: &Token, f: impl FnOnce(&Validator) -> R) -> Option<R> {
        let inner = self.inner.read();
        inner.validators.get(name).map(f)
    }

    /// Return cloned metadata for a specific validator.
    pub fn get_validator_metadata(&self, name: &Token) -> Option<ValidatorMetadata> {
        let inner = self.inner.read();
        inner.validators.get(name).map(|v| v.metadata.clone())
    }

    /// Return all validator metadata.
    pub fn get_all_validator_metadata(&self) -> Vec<ValidatorMetadata> {
        let inner = self.inner.read();
        inner
            .validators
            .values()
            .map(|v| v.metadata.clone())
            .collect()
    }

    /// Return metadata for all validators tagged with the given keyword.
    pub fn get_validator_metadata_for_keyword(&self, keyword: &Token) -> Vec<ValidatorMetadata> {
        let inner = self.inner.read();
        let names = match inner.keyword_index.get(keyword) {
            Some(v) => v,
            None => return Vec::new(),
        };
        names
            .iter()
            .filter_map(|n| inner.validators.get(n).map(|v| v.metadata.clone()))
            .collect()
    }

    /// Return metadata for all validators tagged with the given schema type.
    pub fn get_validator_metadata_for_schema_type(
        &self,
        schema_type: &Token,
    ) -> Vec<ValidatorMetadata> {
        let inner = self.inner.read();
        let names = match inner.schema_type_index.get(schema_type) {
            Some(v) => v,
            None => return Vec::new(),
        };
        names
            .iter()
            .filter_map(|n| inner.validators.get(n).map(|v| v.metadata.clone()))
            .collect()
    }

    /// Names of all registered validators.
    pub fn get_all_validator_names(&self) -> Vec<Token> {
        let inner = self.inner.read();
        inner.validators.keys().cloned().collect()
    }

    /// Run a validator by name against a layer.
    pub fn validate_layer(&self, name: &Token, layer: &Arc<Layer>) -> Vec<ValidationError> {
        let inner = self.inner.read();
        match inner.validators.get(name) {
            Some(v) => v.validate_layer(layer),
            None => Vec::new(),
        }
    }

    /// Run a validator by name against a stage.
    pub fn validate_stage(
        &self,
        name: &Token,
        stage: &Arc<Stage>,
        time_range: &ValidationTimeRange,
    ) -> Vec<ValidationError> {
        let inner = self.inner.read();
        match inner.validators.get(name) {
            Some(v) => v.validate_stage(stage, time_range),
            None => Vec::new(),
        }
    }

    /// Run a validator by name against a prim.
    pub fn validate_prim(
        &self,
        name: &Token,
        prim: &Prim,
        time_range: &ValidationTimeRange,
    ) -> Vec<ValidationError> {
        let inner = self.inner.read();
        match inner.validators.get(name) {
            Some(v) => v.validate_prim(prim, time_range),
            None => Vec::new(),
        }
    }

    // -- Core validator registration (called once at init) --

    fn register_core_validators(&self) {
        self.register_stage_validator(
            ValidatorMetadata::new(tokens::COMPOSITION_ERROR_TEST.clone())
                .with_doc("Checks for composition errors on a stage.".to_string())
                .with_keywords(vec![Token::new("composition"), Token::new("core")]),
            core_validators::composition_error_test_fn(),
            Vec::new(),
        );

        self.register_stage_validator(
            ValidatorMetadata::new(tokens::STAGE_METADATA_CHECKER.clone())
                .with_doc("Checks that the stage has a valid default prim.".to_string())
                .with_keywords(vec![Token::new("metadata"), Token::new("core")]),
            core_validators::stage_metadata_checker_fn(),
            Vec::new(),
        );

        self.register_prim_validator(
            ValidatorMetadata::new(tokens::ATTRIBUTE_TYPE_MISMATCH.clone())
                .with_doc(
                    "Checks for attribute type mismatches across the property stack.".to_string(),
                )
                .with_keywords(vec![Token::new("attribute"), Token::new("core")]),
            core_validators::attribute_type_mismatch_fn(),
            Vec::new(),
        );
    }
}

// ============================================================================
// ValidationContext
// ============================================================================

/// Holds a set of validators selected by keyword, metadata, or explicit list,
/// and runs them in parallel via rayon.
pub struct ValidationContext {
    layer_validators: Vec<Token>,
    stage_validators: Vec<Token>,
    prim_validators: Vec<Token>,
}

impl ValidationContext {
    /// Build a context that includes all validators matching any of `keywords`.
    pub fn from_keywords(keywords: &[Token]) -> Self {
        let reg = ValidationRegistry::get_instance();
        let inner = reg.inner.read();

        let mut layer = Vec::new();
        let mut stage = Vec::new();
        let mut prim = Vec::new();

        for kw in keywords {
            if let Some(names) = inner.keyword_index.get(kw) {
                for name in names {
                    if let Some(v) = inner.validators.get(name) {
                        match &v.task {
                            ValidatorTask::Layer(_) => {
                                if !layer.contains(name) {
                                    layer.push(name.clone());
                                }
                            }
                            ValidatorTask::Stage(_) => {
                                if !stage.contains(name) {
                                    stage.push(name.clone());
                                }
                            }
                            ValidatorTask::Prim(_) => {
                                if !prim.contains(name) {
                                    prim.push(name.clone());
                                }
                            }
                            ValidatorTask::None => {}
                        }
                    }
                }
            }
        }

        Self {
            layer_validators: layer,
            stage_validators: stage,
            prim_validators: prim,
        }
    }

    /// Build a context from explicit metadata list.
    pub fn from_metadata(metas: &[ValidatorMetadata]) -> Self {
        let reg = ValidationRegistry::get_instance();
        let inner = reg.inner.read();

        let mut layer = Vec::new();
        let mut stage = Vec::new();
        let mut prim = Vec::new();

        for m in metas {
            if let Some(v) = inner.validators.get(&m.name) {
                match &v.task {
                    ValidatorTask::Layer(_) => layer.push(m.name.clone()),
                    ValidatorTask::Stage(_) => stage.push(m.name.clone()),
                    ValidatorTask::Prim(_) => prim.push(m.name.clone()),
                    ValidatorTask::None => {}
                }
            }
        }

        Self {
            layer_validators: layer,
            stage_validators: stage,
            prim_validators: prim,
        }
    }

    /// Build a context from explicit validator names.
    pub fn from_names(names: &[Token]) -> Self {
        let reg = ValidationRegistry::get_instance();
        let inner = reg.inner.read();

        let mut layer = Vec::new();
        let mut stage = Vec::new();
        let mut prim = Vec::new();

        for name in names {
            if let Some(v) = inner.validators.get(name) {
                match &v.task {
                    ValidatorTask::Layer(_) => layer.push(name.clone()),
                    ValidatorTask::Stage(_) => stage.push(name.clone()),
                    ValidatorTask::Prim(_) => prim.push(name.clone()),
                    ValidatorTask::None => {}
                }
            }
        }

        Self {
            layer_validators: layer,
            stage_validators: stage,
            prim_validators: prim,
        }
    }

    /// Build a context containing all registered validators.
    pub fn all() -> Self {
        let reg = ValidationRegistry::get_instance();
        let inner = reg.inner.read();

        let mut layer = Vec::new();
        let mut stage = Vec::new();
        let mut prim = Vec::new();

        for (name, v) in inner.validators.iter() {
            match &v.task {
                ValidatorTask::Layer(_) => layer.push(name.clone()),
                ValidatorTask::Stage(_) => stage.push(name.clone()),
                ValidatorTask::Prim(_) => prim.push(name.clone()),
                ValidatorTask::None => {}
            }
        }

        Self {
            layer_validators: layer,
            stage_validators: stage,
            prim_validators: prim,
        }
    }

    /// Validate a layer with all layer validators in this context.
    pub fn validate_layer(&self, layer: &Arc<Layer>) -> Vec<ValidationError> {
        use rayon::prelude::*;

        let reg = ValidationRegistry::get_instance();
        self.layer_validators
            .par_iter()
            .flat_map(|name| reg.validate_layer(name, layer))
            .collect()
    }

    /// Validate a stage with all stage validators in this context.
    pub fn validate_stage(&self, stage: &Arc<Stage>) -> Vec<ValidationError> {
        self.validate_stage_with_range(stage, &ValidationTimeRange::default())
    }

    /// Validate a stage with an explicit time range.
    pub fn validate_stage_with_range(
        &self,
        stage: &Arc<Stage>,
        time_range: &ValidationTimeRange,
    ) -> Vec<ValidationError> {
        use rayon::prelude::*;

        let reg = ValidationRegistry::get_instance();
        self.stage_validators
            .par_iter()
            .flat_map(|name| reg.validate_stage(name, stage, time_range))
            .collect()
    }

    /// Validate a slice of prims with all prim validators in this context.
    pub fn validate_prims(
        &self,
        prims: &[Prim],
        time_range: &ValidationTimeRange,
    ) -> Vec<ValidationError> {
        use rayon::prelude::*;

        let reg = ValidationRegistry::get_instance();
        prims
            .par_iter()
            .flat_map(|prim| {
                self.prim_validators
                    .iter()
                    .flat_map(|name| reg.validate_prim(name, prim, time_range))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Convenience: validate everything on a stage (stage validators + prim
    /// validators on all prims).
    pub fn validate_all(
        &self,
        stage: &Arc<Stage>,
        time_range: &ValidationTimeRange,
    ) -> Vec<ValidationError> {
        let mut errors = self.validate_stage_with_range(stage, time_range);

        // Collect all prims from the stage.
        let root = stage.get_default_prim();
        if root.is_valid() {
            let mut prims = vec![root.clone()];
            Self::collect_prims_recursive(&root, &mut prims);
            errors.extend(self.validate_prims(&prims, time_range));
        }

        errors
    }

    fn collect_prims_recursive(parent: &Prim, out: &mut Vec<Prim>) {
        for child in parent.get_children() {
            out.push(child.clone());
            Self::collect_prims_recursive(&child, out);
        }
    }
}

// ============================================================================
// Tokens (lazy statics)
// ============================================================================

/// Well-known token constants used by the validation framework.
pub mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Composition error test validator name.
    pub static COMPOSITION_ERROR_TEST: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdValidation:CompositionErrorTest"));

    /// Stage metadata checker validator name.
    pub static STAGE_METADATA_CHECKER: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdValidation:StageMetadataChecker"));

    /// Attribute type mismatch validator name.
    pub static ATTRIBUTE_TYPE_MISMATCH: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdValidation:AttributeTypeMismatch"));

    /// Error name: composition error.
    pub static COMPOSITION_ERROR: LazyLock<Token> =
        LazyLock::new(|| Token::new("compositionError"));

    /// Error name: missing default prim.
    pub static MISSING_DEFAULT_PRIM: LazyLock<Token> =
        LazyLock::new(|| Token::new("missingDefaultPrim"));

    /// Error name: attribute type mismatch.
    pub static TYPE_MISMATCH: LazyLock<Token> =
        LazyLock::new(|| Token::new("attributeTypeMismatch"));

    /// Keyword: core.
    pub static KW_CORE: LazyLock<Token> = LazyLock::new(|| Token::new("core"));
    /// Keyword: composition.
    pub static KW_COMPOSITION: LazyLock<Token> = LazyLock::new(|| Token::new("composition"));
    /// Keyword: metadata.
    pub static KW_METADATA: LazyLock<Token> = LazyLock::new(|| Token::new("metadata"));
    /// Keyword: attribute.
    pub static KW_ATTRIBUTE: LazyLock<Token> = LazyLock::new(|| Token::new("attribute"));
}

/// Legacy compat: `ValidatorNameTokens` from the original stub.
pub struct ValidatorNameTokens;

impl ValidatorNameTokens {
    /// Composition error test validator.
    pub fn composition_error_test() -> Token {
        tokens::COMPOSITION_ERROR_TEST.clone()
    }

    /// Stage metadata checker validator.
    pub fn stage_metadata_checker() -> Token {
        tokens::STAGE_METADATA_CHECKER.clone()
    }

    /// Attribute type mismatch validator.
    pub fn attribute_type_mismatch() -> Token {
        tokens::ATTRIBUTE_TYPE_MISMATCH.clone()
    }
}

// ============================================================================
// Core validators
// ============================================================================

/// Implementations of the three built-in validators.
mod core_validators {
    use super::*;

    /// **CompositionErrorTest** -- reports each composition error on the stage.
    pub fn composition_error_test_fn() -> ValidateStageTaskFn {
        Arc::new(|stage: &Arc<Stage>, _time_range: &ValidationTimeRange| {
            let comp_errors = stage.get_composition_errors();
            comp_errors
                .into_iter()
                .map(|msg| {
                    ValidationError::new(
                        tokens::COMPOSITION_ERROR.clone(),
                        ErrorType::Error,
                        vec![ErrorSite::from_stage(stage, Path::absolute_root(), None)],
                        msg,
                    )
                })
                .collect()
        })
    }

    /// **StageMetadataChecker** -- warns if the default prim is invalid.
    pub fn stage_metadata_checker_fn() -> ValidateStageTaskFn {
        Arc::new(|stage: &Arc<Stage>, _time_range: &ValidationTimeRange| {
            let default_prim = stage.get_default_prim();
            if !default_prim.is_valid() || default_prim.is_pseudo_root() {
                vec![ValidationError::new(
                    tokens::MISSING_DEFAULT_PRIM.clone(),
                    ErrorType::Warn,
                    vec![ErrorSite::from_stage(stage, Path::absolute_root(), None)],
                    "Stage has no valid defaultPrim set.".to_string(),
                )]
            } else {
                Vec::new()
            }
        })
    }

    /// **AttributeTypeMismatch** -- for each attribute on a prim, verifies
    /// that the type name from the composed value matches across the
    /// property stack entries.
    pub fn attribute_type_mismatch_fn() -> ValidatePrimTaskFn {
        Arc::new(|prim: &Prim, _time_range: &ValidationTimeRange| {
            let mut errors = Vec::new();
            let attr_names = prim.get_attribute_names();
            for name in &attr_names {
                let attr = match prim.get_attribute(name.as_str()) {
                    Some(a) => a,
                    None => continue,
                };
                // The composed attribute type.
                let composed_type = attr.type_name();
                if composed_type.as_str().is_empty() {
                    continue;
                }
                // Check the SdfValueTypeName for validity.
                let sdf_type = attr.get_type_name();
                if !sdf_type.is_valid() {
                    errors.push(ValidationError::new(
                        tokens::TYPE_MISMATCH.clone(),
                        ErrorType::Error,
                        vec![ErrorSite::from_stage(
                            // We need a stage to build the site; fall back to
                            // empty site if none available.
                            &match attr.stage() {
                                Some(s) => s,
                                None => {
                                    errors.push(ValidationError::simple(
                                        tokens::TYPE_MISMATCH.clone(),
                                        ErrorType::Error,
                                        format!(
                                            "Attribute '{}' on prim '{}' has \
                                             unresolvable type '{}'.",
                                            name.as_str(),
                                            prim.get_path(),
                                            composed_type.as_str()
                                        ),
                                    ));
                                    continue;
                                }
                            },
                            prim.get_path().clone(),
                            None,
                        )],
                        format!(
                            "Attribute '{}' on prim '{}' has invalid type name \
                             '{}'.",
                            name.as_str(),
                            prim.get_path(),
                            composed_type.as_str()
                        ),
                    ));
                }
            }
            errors
        })
    }
}

/// Initialize the validation framework. Forces the singleton to be created
/// and all core validators to be registered. Safe to call multiple times.
pub fn initialize() {
    let _ = ValidationRegistry::get_instance();
}

// ============================================================================
// Skel Validators
// ============================================================================

pub mod skel_validators;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::common::InitialLoadSet;

    // -- ErrorType --

    #[test]
    fn test_error_type_display() {
        assert_eq!(format!("{}", ErrorType::None), "None");
        assert_eq!(format!("{}", ErrorType::Error), "Error");
        assert_eq!(format!("{}", ErrorType::Warn), "Warn");
        assert_eq!(format!("{}", ErrorType::Info), "Info");
    }

    #[test]
    fn test_error_type_equality() {
        assert_eq!(ErrorType::Error, ErrorType::Error);
        assert_ne!(ErrorType::Error, ErrorType::Warn);
    }

    // -- ErrorSite --

    #[test]
    fn test_error_site_empty() {
        let site = ErrorSite::empty();
        assert!(!site.is_valid());
        assert!(!site.is_prim());
        assert!(!site.is_property());
        assert!(site.get_layer().is_none());
        assert!(site.get_stage().is_none());
    }

    #[test]
    fn test_error_site_from_layer() {
        let layer = Layer::create_anonymous(Some("test"));
        let site = ErrorSite::from_layer(&layer, Path::absolute_root());
        assert!(site.is_valid());
        assert!(site.get_layer().is_some());
        assert!(site.get_stage().is_none());
        // "/" is the absolute root, not a prim path per SdfPath semantics.
        assert!(!site.is_prim());
        assert!(!site.is_property());
    }

    #[test]
    fn test_error_site_from_stage() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let site = ErrorSite::from_stage(&stage, Path::absolute_root(), None);
        assert!(site.is_valid());
        assert!(site.get_stage().is_some());
        assert!(site.get_layer().is_none());
    }

    #[test]
    fn test_error_site_from_stage_with_layer() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let layer = stage.get_root_layer();
        let site = ErrorSite::from_stage(&stage, Path::absolute_root(), Some(&layer));
        assert!(site.is_valid());
        assert!(site.get_stage().is_some());
        assert!(site.get_layer().is_some());
    }

    #[test]
    fn test_error_site_equality() {
        let layer = Layer::create_anonymous(Some("eq"));
        let s1 = ErrorSite::from_layer(&layer, Path::absolute_root());
        let s2 = ErrorSite::from_layer(&layer, Path::absolute_root());
        assert_eq!(s1, s2);

        let layer2 = Layer::create_anonymous(Some("eq2"));
        let s3 = ErrorSite::from_layer(&layer2, Path::absolute_root());
        assert_ne!(s1, s3);
    }

    // -- ValidationError --

    #[test]
    fn test_error_creation() {
        let err = ValidationError::new(
            Token::new("testErr"),
            ErrorType::Error,
            Vec::new(),
            "something broke".to_string(),
        );
        assert_eq!(err.get_name().as_str(), "testErr");
        assert_eq!(err.get_type(), ErrorType::Error);
        assert_eq!(err.get_message(), "something broke");
        assert!(err.get_sites().is_empty());
        assert!(err.get_data().is_none());
    }

    #[test]
    fn test_error_simple() {
        let err = ValidationError::simple(Token::new("x"), ErrorType::Warn, "warn msg".to_string());
        assert_eq!(err.get_type(), ErrorType::Warn);
        assert!(err.get_sites().is_empty());
    }

    #[test]
    fn test_error_has_no_error() {
        let ok = ValidationError::simple(Token::new("ok"), ErrorType::None, String::new());
        assert!(ok.has_no_error());

        let fail = ValidationError::simple(Token::new("fail"), ErrorType::Error, String::new());
        assert!(!fail.has_no_error());
    }

    #[test]
    fn test_error_identifier_without_validator() {
        let err = ValidationError::simple(Token::new("myErr"), ErrorType::Error, "msg".to_string());
        assert_eq!(err.get_identifier(), "myErr");
    }

    #[test]
    fn test_error_identifier_with_validator() {
        let err = ValidationError::simple(Token::new("myErr"), ErrorType::Error, "msg".to_string())
            .with_validator(Token::new("myValidator"));
        assert_eq!(err.get_identifier(), "myValidator: myErr");
    }

    #[test]
    fn test_error_as_string() {
        let err = ValidationError::simple(Token::new("e1"), ErrorType::Error, "broken".to_string())
            .with_validator(Token::new("v1"));
        let s = err.get_error_as_string();
        assert!(s.contains("[Error]"));
        assert!(s.contains("v1: e1"));
        assert!(s.contains("broken"));
    }

    #[test]
    fn test_error_equality() {
        let e1 = ValidationError::simple(Token::new("a"), ErrorType::Error, "msg".to_string());
        let e2 = ValidationError::simple(Token::new("a"), ErrorType::Error, "msg".to_string());
        assert_eq!(e1, e2);

        let e3 = ValidationError::simple(Token::new("b"), ErrorType::Error, "msg".to_string());
        assert_ne!(e1, e3);
    }

    // -- ValidationTimeRange --

    #[test]
    fn test_time_range_default() {
        let tr = ValidationTimeRange::default();
        assert!(tr.includes_time_code_default());
        let iv = tr.get_interval();
        assert_eq!(iv.get_min(), f64::NEG_INFINITY);
        assert_eq!(iv.get_max(), f64::INFINITY);
    }

    #[test]
    fn test_time_range_from_time_code() {
        let tc = TimeCode::new(5.0);
        let tr = ValidationTimeRange::from_time_code(&tc);
        assert!(!tr.includes_time_code_default());
        assert!(tr.get_interval().contains(5.0));
    }

    #[test]
    fn test_time_range_from_default_time_code() {
        let tc = TimeCode::default();
        let tr = ValidationTimeRange::from_time_code(&tc);
        assert!(tr.includes_time_code_default());
    }

    #[test]
    fn test_time_range_from_interval() {
        let iv = Interval::closed(1.0, 10.0);
        let tr = ValidationTimeRange::from_interval(iv, false);
        assert!(!tr.includes_time_code_default());
        assert!(tr.get_interval().contains(5.0));
        assert!(!tr.get_interval().contains(11.0));
    }

    // -- ValidationFixer --

    #[test]
    fn test_fixer_basics() {
        let fixer = ValidationFixer::new(
            Token::new("fix1"),
            "Fix the thing".to_string(),
            Token::new("err1"),
            vec![Token::new("kw1"), Token::new("kw2")],
            Arc::new(|_e, _t, _tc| true),
            Arc::new(|_e, _t, _tc| true),
        );
        assert_eq!(fixer.get_name().as_str(), "fix1");
        assert_eq!(fixer.get_description(), "Fix the thing");
        assert_eq!(fixer.get_error_name().as_str(), "err1");
        assert!(fixer.is_associated_with_error_name(&Token::new("err1")));
        assert!(!fixer.is_associated_with_error_name(&Token::new("err2")));
        assert!(fixer.has_keyword(&Token::new("kw1")));
        assert!(!fixer.has_keyword(&Token::new("kw3")));
        assert_eq!(fixer.get_keywords().len(), 2);
    }

    #[test]
    fn test_fixer_apply() {
        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = Arc::clone(&call_count);
        let fixer = ValidationFixer::new(
            Token::new("f"),
            String::new(),
            Token::new("e"),
            Vec::new(),
            Arc::new(move |_e, _t, _tc| {
                cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                true
            }),
            Arc::new(|_e, _t, _tc| true),
        );
        let err = ValidationError::simple(Token::new("e"), ErrorType::Error, String::new());
        let et = EditTarget::default();
        let tc = TimeCode::default();
        assert!(fixer.can_apply_fix(&err, &et, &tc));
        assert!(fixer.apply_fix(&err, &et, &tc));
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    // -- ValidatorMetadata --

    #[test]
    fn test_metadata_builder() {
        let m = ValidatorMetadata::new(Token::new("v1"))
            .with_doc("my doc")
            .with_keywords(vec![Token::new("k1")])
            .with_schema_types(vec![Token::new("Mesh")])
            .with_time_dependent(true);
        assert_eq!(m.name.as_str(), "v1");
        assert_eq!(m.doc, "my doc");
        assert_eq!(m.keywords.len(), 1);
        assert_eq!(m.schema_types.len(), 1);
        assert!(m.is_time_dependent);
        assert!(!m.is_suite);
    }

    #[test]
    fn test_metadata_suite() {
        let m = ValidatorMetadata::new(Token::new("s1")).as_suite();
        assert!(m.is_suite);
    }

    // -- Validator --

    #[test]
    fn test_validator_layer() {
        let v = Validator::new(
            ValidatorMetadata::new(Token::new("lv")),
            ValidatorTask::Layer(Arc::new(|_layer| {
                vec![ValidationError::simple(
                    Token::new("le"),
                    ErrorType::Info,
                    "layer info".to_string(),
                )]
            })),
            Vec::new(),
        );
        assert!(v.is_layer_validator());
        assert!(!v.is_stage_validator());
        assert!(!v.is_prim_validator());
        let layer = Layer::create_anonymous(Some("lv_test"));
        let errs = v.validate_layer(&layer);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].get_validator_name().as_str(), "lv");
    }

    #[test]
    fn test_validator_stage() {
        let v = Validator::new(
            ValidatorMetadata::new(Token::new("sv")),
            ValidatorTask::Stage(Arc::new(|_stage, _tr| Vec::new())),
            Vec::new(),
        );
        assert!(v.is_stage_validator());
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let errs = v.validate_stage(&stage, &ValidationTimeRange::default());
        assert!(errs.is_empty());
    }

    #[test]
    fn test_validator_wrong_type_returns_empty() {
        let v = Validator::new(
            ValidatorMetadata::new(Token::new("pv")),
            ValidatorTask::Prim(Arc::new(|_p, _tr| Vec::new())),
            Vec::new(),
        );
        // Calling validate_layer on a prim validator returns empty.
        let layer = Layer::create_anonymous(Some("x"));
        assert!(v.validate_layer(&layer).is_empty());
    }

    // -- ValidatorSuite --

    #[test]
    fn test_suite() {
        let suite = ValidatorSuite::new(
            ValidatorMetadata::new(Token::new("suite1")).as_suite(),
            vec![Token::new("v1"), Token::new("v2")],
        );
        assert_eq!(suite.get_metadata().name.as_str(), "suite1");
        assert!(suite.get_metadata().is_suite);
        assert_eq!(suite.get_contained_validators().len(), 2);
    }

    // -- Tokens --

    #[test]
    fn test_token_constants() {
        assert_eq!(
            tokens::COMPOSITION_ERROR_TEST.as_str(),
            "usdValidation:CompositionErrorTest"
        );
        assert_eq!(
            tokens::STAGE_METADATA_CHECKER.as_str(),
            "usdValidation:StageMetadataChecker"
        );
        assert_eq!(
            tokens::ATTRIBUTE_TYPE_MISMATCH.as_str(),
            "usdValidation:AttributeTypeMismatch"
        );
    }

    #[test]
    fn test_validator_name_tokens_compat() {
        assert_eq!(
            ValidatorNameTokens::composition_error_test().as_str(),
            "usdValidation:CompositionErrorTest"
        );
        assert_eq!(
            ValidatorNameTokens::stage_metadata_checker().as_str(),
            "usdValidation:StageMetadataChecker"
        );
    }

    // -- ValidationRegistry --

    #[test]
    fn test_registry_singleton() {
        let r1 = ValidationRegistry::get_instance();
        let r2 = ValidationRegistry::get_instance();
        assert!(std::ptr::eq(r1, r2));
    }

    #[test]
    fn test_registry_has_core_validators() {
        let reg = ValidationRegistry::get_instance();
        assert!(reg.has_validator(&tokens::COMPOSITION_ERROR_TEST));
        assert!(reg.has_validator(&tokens::STAGE_METADATA_CHECKER));
        assert!(reg.has_validator(&tokens::ATTRIBUTE_TYPE_MISMATCH));
    }

    #[test]
    fn test_registry_has_no_unknown() {
        let reg = ValidationRegistry::get_instance();
        assert!(!reg.has_validator(&Token::new("nonExistent")));
    }

    #[test]
    fn test_registry_metadata_lookup() {
        let reg = ValidationRegistry::get_instance();
        let meta = reg
            .get_validator_metadata(&tokens::COMPOSITION_ERROR_TEST)
            .unwrap();
        assert_eq!(meta.name, *tokens::COMPOSITION_ERROR_TEST);
        assert!(meta.doc.contains("composition"));
    }

    #[test]
    fn test_registry_all_metadata() {
        let reg = ValidationRegistry::get_instance();
        let all = reg.get_all_validator_metadata();
        // At least the 3 core validators.
        assert!(all.len() >= 3);
    }

    #[test]
    fn test_registry_keyword_lookup() {
        let reg = ValidationRegistry::get_instance();
        let metas = reg.get_validator_metadata_for_keyword(&Token::new("core"));
        assert!(metas.len() >= 3);
    }

    #[test]
    fn test_registry_keyword_empty() {
        let reg = ValidationRegistry::get_instance();
        let metas = reg.get_validator_metadata_for_keyword(&Token::new("nonexistent_kw"));
        assert!(metas.is_empty());
    }

    #[test]
    fn test_registry_with_validator() {
        let reg = ValidationRegistry::get_instance();
        let is_stage = reg
            .with_validator(&tokens::COMPOSITION_ERROR_TEST, |v| v.is_stage_validator())
            .unwrap();
        assert!(is_stage);
    }

    #[test]
    fn test_registry_suite() {
        let reg = ValidationRegistry::get_instance();
        // Register a test suite.
        reg.register_validator_suite(
            ValidatorMetadata::new(Token::new("testSuite:all")).as_suite(),
            vec![
                tokens::COMPOSITION_ERROR_TEST.clone(),
                tokens::STAGE_METADATA_CHECKER.clone(),
            ],
        );
        assert!(reg.has_validator_suite(&Token::new("testSuite:all")));
    }

    #[test]
    fn test_registry_custom_validator() {
        let reg = ValidationRegistry::get_instance();
        let name = Token::new("test:customLayer");
        reg.register_layer_validator(
            ValidatorMetadata::new(name.clone()).with_doc("custom test validator"),
            Arc::new(|layer: &Arc<Layer>| {
                let id = layer.identifier();
                if id.is_empty() {
                    vec![ValidationError::simple(
                        Token::new("emptyId"),
                        ErrorType::Warn,
                        "Layer has empty identifier.".to_string(),
                    )]
                } else {
                    Vec::new()
                }
            }),
            Vec::new(),
        );
        assert!(reg.has_validator(&name));

        // Validate a real layer.
        let layer = Layer::create_anonymous(Some("custom_test"));
        let errs = reg.validate_layer(&name, &layer);
        // Anonymous layers have an identifier, so no errors expected.
        assert!(errs.is_empty());
    }

    // -- Core validators: CompositionErrorTest --

    #[test]
    fn test_composition_error_test_clean_stage() {
        let reg = ValidationRegistry::get_instance();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let tr = ValidationTimeRange::default();
        let errs = reg.validate_stage(&tokens::COMPOSITION_ERROR_TEST, &stage, &tr);
        // A fresh in-memory stage has no composition errors.
        assert!(errs.is_empty());
    }

    // -- Core validators: StageMetadataChecker --

    #[test]
    fn test_stage_metadata_checker_no_default_prim() {
        let reg = ValidationRegistry::get_instance();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let tr = ValidationTimeRange::default();
        let errs = reg.validate_stage(&tokens::STAGE_METADATA_CHECKER, &stage, &tr);
        // Empty stage has no default prim -> expect a warning.
        assert!(!errs.is_empty());
        assert_eq!(errs[0].get_type(), ErrorType::Warn);
        assert!(errs[0].get_message().contains("defaultPrim"));
    }

    // -- ValidationContext --

    #[test]
    fn test_context_from_keywords() {
        let ctx = ValidationContext::from_keywords(&[Token::new("core")]);
        // Should have stage and prim validators.
        assert!(!ctx.stage_validators.is_empty());
        assert!(!ctx.prim_validators.is_empty());
    }

    #[test]
    fn test_context_all() {
        let ctx = ValidationContext::all();
        // At least the 3 core validators split between stage and prim.
        let total =
            ctx.layer_validators.len() + ctx.stage_validators.len() + ctx.prim_validators.len();
        assert!(total >= 3);
    }

    #[test]
    fn test_context_validate_stage() {
        let ctx = ValidationContext::from_names(&[
            tokens::COMPOSITION_ERROR_TEST.clone(),
            tokens::STAGE_METADATA_CHECKER.clone(),
        ]);
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let errs = ctx.validate_stage(&stage);
        // Stage metadata checker should fire (no default prim).
        assert!(
            errs.iter()
                .any(|e| e.get_name() == &*tokens::MISSING_DEFAULT_PRIM)
        );
    }

    #[test]
    fn test_context_validate_layer() {
        // Register a layer validator for this test.
        let reg = ValidationRegistry::get_instance();
        let name = Token::new("test:layerCtx");
        reg.register_layer_validator(
            ValidatorMetadata::new(name.clone()).with_keywords(vec![Token::new("layerCtxTest")]),
            Arc::new(|_layer: &Arc<Layer>| Vec::new()),
            Vec::new(),
        );

        let ctx = ValidationContext::from_keywords(&[Token::new("layerCtxTest")]);
        assert_eq!(ctx.layer_validators.len(), 1);
        let layer = Layer::create_anonymous(Some("ctx_test"));
        let errs = ctx.validate_layer(&layer);
        assert!(errs.is_empty());
    }

    #[test]
    fn test_context_from_metadata() {
        let metas = vec![ValidatorMetadata::new(
            tokens::STAGE_METADATA_CHECKER.clone(),
        )];
        let ctx = ValidationContext::from_metadata(&metas);
        assert_eq!(ctx.stage_validators.len(), 1);
    }

    #[test]
    fn test_context_from_names_unknown() {
        let ctx = ValidationContext::from_names(&[Token::new("does_not_exist")]);
        assert!(ctx.layer_validators.is_empty());
        assert!(ctx.stage_validators.is_empty());
        assert!(ctx.prim_validators.is_empty());
    }

    // -- Initialize --

    #[test]
    fn test_initialize() {
        // Should not panic, even if called multiple times.
        initialize();
        initialize();
    }
}
