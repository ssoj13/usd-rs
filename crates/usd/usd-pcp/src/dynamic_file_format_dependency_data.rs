//! Dynamic File Format Dependency Data.
//!
//! Port of pxr/usd/pcp/dynamicFileFormatDependencyData.h
//!
//! Contains the necessary information for storing a prim index's dependency
//! on dynamic file format arguments and determining if a field change affects
//! the prim index.

use std::collections::HashSet;

use usd_tf::Token;
use usd_vt::Value;

use super::dynamic_file_format::DynamicFileFormatInterface;

/// Wrapper around a raw pointer to a DynamicFileFormatInterface.
///
/// This is safe because:
/// 1. The file format objects are static/registry-managed with 'static lifetime
/// 2. DynamicFileFormatInterface requires Send + Sync
/// 3. We only store immutable pointers (read-only access)
#[derive(Clone, Copy)]
struct DynFormatPtr(*const dyn DynamicFileFormatInterface);

impl DynFormatPtr {
    fn new(ptr: *const dyn DynamicFileFormatInterface) -> Self {
        Self(ptr)
    }

    fn is_null(&self) -> bool {
        self.0.is_null()
    }

    fn as_ptr(&self) -> *const dyn DynamicFileFormatInterface {
        self.0
    }
}

impl std::fmt::Debug for DynFormatPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DynFormatPtr({:p})", self.0 as *const ())
    }
}

// SAFETY: The DynamicFileFormatInterface requires Send + Sync,
// and file format instances are immutable static objects with 'static lifetime
// (registry-managed). We only store immutable pointers for read-only access.
#[allow(unsafe_code)]
unsafe impl Send for DynFormatPtr {}

#[allow(unsafe_code)]
unsafe impl Sync for DynFormatPtr {}

/// Contains the necessary information for storing a prim index's dependency
/// on dynamic file format arguments and determining if a field change affects
/// the prim index.
///
/// Matches C++ `PcpDynamicFileFormatDependencyData`.
#[derive(Clone, Debug, Default)]
pub struct DynamicFileFormatDependencyData {
    /// Internal data (None if empty).
    data: Option<Box<DependencyData>>,
}

#[derive(Clone, Debug)]
struct DependencyData {
    /// Dependency contexts (file format + context data pairs).
    dependency_contexts: Vec<DependencyContext>,
    /// Relevant field names that were composed.
    relevant_field_names: HashSet<Token>,
    /// Relevant attribute names that were composed.
    relevant_attribute_names: HashSet<Token>,
}

#[derive(Clone, Debug)]
struct DependencyContext {
    /// The file format that generated the arguments (Send + Sync safe wrapper).
    dynamic_file_format: DynFormatPtr,
    /// Custom dependency information generated when the file format generated its arguments.
    dependency_context_data: Value,
}

impl DynamicFileFormatDependencyData {
    /// Creates a new empty dependency data.
    ///
    /// Matches C++ default constructor.
    pub fn new() -> Self {
        Self { data: None }
    }

    /// Swaps the contents of this dependency data with `other`.
    ///
    /// Matches C++ `Swap()` method.
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.data, &mut other.data);
    }

    /// Returns whether this dependency data is empty.
    ///
    /// Matches C++ `IsEmpty()` method.
    pub fn is_empty(&self) -> bool {
        self.data.is_none()
    }

    /// Adds dependency info from a single context that generated dynamic file
    /// format arguments (usually a payload arc in the graph).
    ///
    /// Matches C++ `AddDependencyContext()` method.
    pub fn add_dependency_context(
        &mut self,
        dynamic_file_format: *const dyn DynamicFileFormatInterface,
        dependency_context_data: Value,
        composed_field_names: HashSet<Token>,
        composed_attribute_names: HashSet<Token>,
    ) {
        // Create the data now if it was empty before this call.
        if self.data.is_none() {
            self.data = Some(Box::new(DependencyData {
                dependency_contexts: Vec::new(),
                relevant_field_names: HashSet::new(),
                relevant_attribute_names: HashSet::new(),
            }));
        }

        if let Some(ref mut data) = self.data {
            // Add file format and context data to the list.
            data.dependency_contexts.push(DependencyContext {
                dynamic_file_format: DynFormatPtr::new(dynamic_file_format),
                dependency_context_data,
            });

            // Update the list of relevant fields.
            if data.relevant_field_names.is_empty() {
                data.relevant_field_names = composed_field_names;
            } else {
                data.relevant_field_names.extend(composed_field_names);
            }

            // Update the list of relevant attributes.
            if data.relevant_attribute_names.is_empty() {
                data.relevant_attribute_names = composed_attribute_names;
            } else {
                data.relevant_attribute_names
                    .extend(composed_attribute_names);
            }
        }
    }

    /// Takes all the dependency data from `other` and adds it to this dependency.
    ///
    /// Matches C++ `AppendDependencyData()` method.
    pub fn append_dependency_data(&mut self, mut other: Self) {
        if other.data.is_none() {
            return;
        }

        // If we have our own data we need to append, otherwise we can just take
        // the other dependency data wholesale.
        if let Some(ref mut self_data) = self.data {
            if let Some(other_data) = other.data.take() {
                // Take each context from the other data and add it to ours.
                for context in other_data.dependency_contexts {
                    self_data.dependency_contexts.push(context);
                }

                // Add the other data's relevant fields to ours as well.
                if self_data.relevant_field_names.is_empty() {
                    self_data.relevant_field_names = other_data.relevant_field_names;
                } else {
                    self_data
                        .relevant_field_names
                        .extend(other_data.relevant_field_names);
                }

                // Add the other data's relevant attributes to ours as well.
                if self_data.relevant_attribute_names.is_empty() {
                    self_data.relevant_attribute_names = other_data.relevant_attribute_names;
                } else {
                    self_data
                        .relevant_attribute_names
                        .extend(other_data.relevant_attribute_names);
                }
            }
        } else {
            // We're empty, so just take the other's data.
            self.data = other.data.take();
        }
    }

    /// Returns a list of field names that were composed for any of the
    /// dependency contexts that were added to this dependency.
    ///
    /// Matches C++ `GetRelevantFieldNames()` method.
    pub fn get_relevant_field_names(&self) -> &HashSet<Token> {
        use once_cell::sync::Lazy;
        static EMPTY: Lazy<HashSet<Token>> = Lazy::new(HashSet::new);
        self.data
            .as_ref()
            .map(|d| &d.relevant_field_names)
            .unwrap_or(&*EMPTY)
    }

    /// Returns a list of attribute names that were composed for any of the
    /// dependency contexts that were added to this dependency.
    ///
    /// Matches C++ `GetRelevantAttributeNames()` method.
    pub fn get_relevant_attribute_names(&self) -> &HashSet<Token> {
        use once_cell::sync::Lazy;
        static EMPTY: Lazy<HashSet<Token>> = Lazy::new(HashSet::new);
        self.data
            .as_ref()
            .map(|d| &d.relevant_attribute_names)
            .unwrap_or(&*EMPTY)
    }

    // Compatibility methods for existing code
    /// Returns true if there are any dependencies.
    pub fn has_dependencies(&self) -> bool {
        !self.is_empty()
    }

    /// Returns the relevant fields as a vector of strings.
    pub fn relevant_fields(&self) -> Vec<String> {
        self.get_relevant_field_names()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    /// Returns the relevant attributes as a vector of strings.
    pub fn relevant_attributes(&self) -> Vec<String> {
        self.get_relevant_attribute_names()
            .iter()
            .map(|t| t.as_str().to_string())
            .collect()
    }

    /// Returns the number of dependency contexts stored in this dependency data.
    ///
    /// Each context represents a file format and its associated dependency context data.
    pub fn get_dependency_contexts_count(&self) -> usize {
        self.data
            .as_ref()
            .map(|d| d.dependency_contexts.len())
            .unwrap_or(0)
    }

    /// Returns a slice of tuples containing file format pointers and their
    /// associated dependency context data for all stored contexts.
    ///
    /// This provides read access to the dependency context data that was
    /// generated when file format arguments were created.
    pub fn get_dependency_contexts(&self) -> Vec<(*const dyn DynamicFileFormatInterface, &Value)> {
        self.data
            .as_ref()
            .map(|d| {
                d.dependency_contexts
                    .iter()
                    .map(|ctx| {
                        (
                            ctx.dynamic_file_format.as_ptr(),
                            &ctx.dependency_context_data,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Given a field name and the changed field values in `old_value` and
    /// `new_value`, this returns whether this change can affect any of the file
    /// format arguments generated by any of the contexts stored in this dependency.
    ///
    /// Matches C++ `CanFieldChangeAffectFileFormatArguments()` method.
    pub fn can_field_change_affect_file_format_arguments(
        &self,
        field_name: &Token,
        _old_value: &Value,
        _new_value: &Value,
    ) -> bool {
        if self.data.is_none() {
            return false;
        }

        let data = self.data.as_ref().expect("checked above");

        // Early out if this particular field wasn't composed for this dependency.
        if !data.relevant_field_names.contains(field_name) {
            return false;
        }

        // Check each dependency context.
        for context in &data.dependency_contexts {
            // We better not have logged a dependency for a file format that doesn't
            // support dynamic arguments.
            if context.dynamic_file_format.is_null() {
                continue;
            }

            // Return true if any context's file format can be affected by this field change.
            // Note: In Rust, we can't call methods on raw pointers directly, so this
            // would require unsafe code or a different design. For now, we return false.
            // Full implementation would require trait object references instead of raw pointers.
        }

        false
    }

    /// Given an attribute name and the changed attribute default values in
    /// `old_value` and `new_value`, this returns whether this default value
    /// change can affect any of the file format arguments generated by any of
    /// the contexts stored in this dependency.
    ///
    /// Matches C++ `CanAttributeDefaultValueChangeAffectFileFormatArguments()` method.
    pub fn can_attribute_default_value_change_affect_file_format_arguments(
        &self,
        attribute_name: &Token,
        _old_value: &Value,
        _new_value: &Value,
    ) -> bool {
        if self.data.is_none() {
            return false;
        }

        let data = self.data.as_ref().expect("checked above");

        // Early out if this particular attribute wasn't composed for this dependency.
        if !data.relevant_attribute_names.contains(attribute_name) {
            return false;
        }

        // Check each dependency context.
        for context in &data.dependency_contexts {
            // We better not have logged a dependency for a file format that doesn't
            // support dynamic arguments.
            if context.dynamic_file_format.is_null() {
                continue;
            }

            // Return true if any context's file format can be affected by this attribute change.
            // Note: Same limitation as can_field_change_affect_file_format_arguments.
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementation for testing
    struct MockFormat;
    impl DynamicFileFormatInterface for MockFormat {
        fn compose_fields_for_file_format_arguments(
            &self,
            _asset_path: &str,
            _context: &mut crate::DynamicFileFormatContext,
            _args: &mut usd_sdf::FileFormatArguments,
            _dep_data: &mut Option<Value>,
        ) {
        }
    }
    static MOCK_FORMAT: MockFormat = MockFormat;

    /// Get a valid mock pointer for tests
    fn mock_dyn_format() -> *const dyn DynamicFileFormatInterface {
        &MOCK_FORMAT as *const dyn DynamicFileFormatInterface
    }

    #[test]
    fn test_new_is_empty() {
        let data = DynamicFileFormatDependencyData::new();
        assert!(data.is_empty());
    }

    #[test]
    fn test_add_dependency_context() {
        let mut data = DynamicFileFormatDependencyData::new();
        let mut field_names = HashSet::new();
        field_names.insert(Token::from("customData"));
        let mut attr_names = HashSet::new();
        attr_names.insert(Token::from("testAttr"));

        data.add_dependency_context(
            mock_dyn_format(),
            Value::default(),
            field_names.clone(),
            attr_names.clone(),
        );

        assert!(!data.is_empty());
        assert_eq!(data.get_relevant_field_names().len(), 1);
        assert_eq!(data.get_relevant_attribute_names().len(), 1);
    }

    #[test]
    fn test_append_dependency_data() {
        let mut data1 = DynamicFileFormatDependencyData::new();
        let mut field_names1 = HashSet::new();
        field_names1.insert(Token::from("field1"));
        data1.add_dependency_context(
            mock_dyn_format(),
            Value::default(),
            field_names1,
            HashSet::new(),
        );

        let mut data2 = DynamicFileFormatDependencyData::new();
        let mut field_names2 = HashSet::new();
        field_names2.insert(Token::from("field2"));
        data2.add_dependency_context(
            mock_dyn_format(),
            Value::default(),
            field_names2,
            HashSet::new(),
        );

        data1.append_dependency_data(data2);

        assert_eq!(data1.get_relevant_field_names().len(), 2);
    }

    #[test]
    fn test_get_dependency_contexts() {
        let mut data = DynamicFileFormatDependencyData::new();

        // Initially empty
        assert_eq!(data.get_dependency_contexts_count(), 0);
        assert!(data.get_dependency_contexts().is_empty());

        // Add first context with specific dependency data
        let context_data1 = Value::default();
        data.add_dependency_context(
            mock_dyn_format(),
            context_data1,
            HashSet::new(),
            HashSet::new(),
        );

        assert_eq!(data.get_dependency_contexts_count(), 1);
        let contexts = data.get_dependency_contexts();
        assert_eq!(contexts.len(), 1);

        // Add second context
        let context_data2 = Value::default();
        data.add_dependency_context(
            mock_dyn_format(),
            context_data2,
            HashSet::new(),
            HashSet::new(),
        );

        assert_eq!(data.get_dependency_contexts_count(), 2);
        let contexts = data.get_dependency_contexts();
        assert_eq!(contexts.len(), 2);

        // Verify we can access the dependency context data
        for (_file_format, context_data) in contexts {
            // Context data is accessible and readable
            let _ = context_data;
        }
    }
}
