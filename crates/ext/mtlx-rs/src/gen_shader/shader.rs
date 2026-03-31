//! Shader, ShaderStage — result of shader generation.

use std::collections::{HashMap, HashSet};

use crate::core::Value;
use crate::format::{FilePath, read_file};

use super::gen_context::ShaderImplContext;
use super::shader_graph::ShaderGraph;
use super::type_desc::TypeDesc;

/// Indentation string used by all generators (matches C++ Syntax::getIndentation = 4 spaces).
pub const INDENTATION_STRING: &str = "    ";

/// Punctuation type for scope begin/end (matches C++ Syntax::Punctuation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopePunctuation {
    CurlyBrackets,
    Parentheses,
    SquareBrackets,
    DoubleSquareBrackets,
}

impl Default for ScopePunctuation {
    fn default() -> Self {
        Self::CurlyBrackets
    }
}

/// Stage identifiers
pub mod stage {
    pub const PIXEL: &str = "pixel";
    pub const VERTEX: &str = "vertex";
}

/// Variable block — uniforms, inputs, or outputs in a stage (по рефу VariableBlock)
#[derive(Clone, Debug)]
pub struct VariableBlock {
    pub name: String,
    pub instance: String,
    pub variables: Vec<super::shader_node::ShaderPort>,
    pub variable_order: Vec<String>,
    variable_map: HashMap<String, usize>,
}

impl VariableBlock {
    pub fn new(name: impl Into<String>, instance: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            instance: instance.into(),
            variables: Vec::new(),
            variable_order: Vec::new(),
            variable_map: HashMap::new(),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_instance(&self) -> &str {
        &self.instance
    }

    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    pub fn size(&self) -> usize {
        self.variables.len()
    }

    /// Add a variable to the block (по рефу VariableBlock::add). Returns index of the port.
    pub fn add(
        &mut self,
        type_desc: TypeDesc,
        name: impl Into<String>,
        value: Option<Value>,
        should_widen: bool,
    ) -> &mut super::shader_node::ShaderPort {
        let name = name.into();
        if let Some(&idx) = self.variable_map.get(&name) {
            if should_widen {
                let existing = &mut self.variables[idx];
                if existing.type_desc.get_size() < type_desc.get_size() {
                    existing.type_desc = type_desc;
                }
            }
            return &mut self.variables[idx];
        }
        let mut port = super::shader_node::ShaderPort::new(type_desc, &name);
        if let Some(v) = value {
            port.set_value(Some(v), false);
        }
        let idx = self.variables.len();
        self.variables.push(port);
        self.variable_order.push(name.clone());
        self.variable_map.insert(name, idx);
        &mut self.variables[idx]
    }

    /// Find variable by name. Returns None if not found.
    pub fn find(&self, name: &str) -> Option<&super::shader_node::ShaderPort> {
        self.variable_map.get(name).map(|&idx| &self.variables[idx])
    }

    /// Find variable by name (mutable).
    pub fn find_mut(&mut self, name: &str) -> Option<&mut super::shader_node::ShaderPort> {
        self.variable_map
            .get(name)
            .copied()
            .map(move |idx| &mut self.variables[idx])
    }

    /// Get variable by index (по рефу operator[](size_t)).
    pub fn get(&self, index: usize) -> Option<&super::shader_node::ShaderPort> {
        self.variables.get(index)
    }

    /// Get variable by index (mutable).
    pub fn get_mut(&mut self, index: usize) -> Option<&mut super::shader_node::ShaderPort> {
        self.variables.get_mut(index)
    }

    /// Get variable order (names in declaration order).
    pub fn get_variable_order(&self) -> &[String] {
        &self.variable_order
    }

    /// Find first variable matching a predicate (C++ VariableBlock::find(pred)).
    pub fn find_with_predicate<F>(&self, pred: F) -> Option<&super::shader_node::ShaderPort>
    where
        F: Fn(&super::shader_node::ShaderPort) -> bool,
    {
        self.variables.iter().find(|p| pred(p))
    }

    /// Find first variable (mutable) matching a predicate.
    pub fn find_with_predicate_mut<F>(
        &mut self,
        pred: F,
    ) -> Option<&mut super::shader_node::ShaderPort>
    where
        F: Fn(&super::shader_node::ShaderPort) -> bool,
    {
        self.variables.iter_mut().find(|p| pred(p))
    }
}

/// Shader stage — vertex, pixel, etc. Holds source code and variable blocks.
#[derive(Clone, Debug)]
pub struct ShaderStage {
    pub name: String,
    pub function_name: String,
    pub source_code: String,
    pub uniforms: HashMap<String, VariableBlock>,
    pub inputs: HashMap<String, VariableBlock>,
    pub outputs: HashMap<String, VariableBlock>,
    pub constants: VariableBlock,
    pub includes: HashSet<String>,
    /// Source files already emitted (avoid duplicate function definitions).
    pub source_dependencies: HashSet<String>,
    /// Node names whose function calls have been emitted (per scope tracking).
    pub emitted_function_calls: HashSet<String>,
    /// Current indentation level (matches C++ ShaderStage::_indentations).
    pub indentation: usize,
    /// Stack of open scopes (matches C++ ShaderStage::_scopes).
    pub scopes: Vec<ScopePunctuation>,
    /// Hash IDs of functions already defined in this stage (hash-based dedup).
    pub defined_functions: HashSet<u64>,
    /// Name-based function definition dedup (matches C++ ShaderStage::_definedFunctions).
    pub defined_functions_by_name: HashSet<String>,
}

impl ShaderStage {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            function_name: String::new(),
            source_code: String::new(),
            uniforms: HashMap::new(),
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            constants: VariableBlock::new("constants", ""),
            includes: HashSet::new(),
            source_dependencies: HashSet::new(),
            emitted_function_calls: HashSet::new(),
            indentation: 0,
            scopes: Vec::new(),
            defined_functions: HashSet::new(),
            defined_functions_by_name: HashSet::new(),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_function_name(&self) -> &str {
        &self.function_name
    }

    pub fn set_function_name(&mut self, name: impl Into<String>) {
        self.function_name = name.into();
    }

    pub fn set_source_code(&mut self, code: impl Into<String>) {
        self.source_code = code.into();
    }

    pub fn get_source_code(&self) -> &str {
        &self.source_code
    }

    pub fn create_uniform_block(
        &mut self,
        name: impl Into<String>,
        instance: impl Into<String>,
    ) -> &mut VariableBlock {
        let name = name.into();
        self.uniforms
            .entry(name.clone())
            .or_insert_with(|| VariableBlock::new(name, instance.into()))
    }

    pub fn create_input_block(
        &mut self,
        name: impl Into<String>,
        instance: impl Into<String>,
    ) -> &mut VariableBlock {
        let name = name.into();
        self.inputs
            .entry(name.clone())
            .or_insert_with(|| VariableBlock::new(name, instance.into()))
    }

    pub fn create_output_block(
        &mut self,
        name: impl Into<String>,
        instance: impl Into<String>,
    ) -> &mut VariableBlock {
        let name = name.into();
        self.outputs
            .entry(name.clone())
            .or_insert_with(|| VariableBlock::new(name, instance.into()))
    }

    pub fn add_include(&mut self, path: impl Into<String>) {
        self.includes.insert(path.into());
    }

    /// Append source code (for shader generation)
    pub fn append_source_code(&mut self, code: impl AsRef<str>) {
        self.source_code.push_str(code.as_ref());
    }

    /// Append a line of code with newline
    pub fn append_line(&mut self, line: impl AsRef<str>) {
        self.source_code.push_str(line.as_ref());
        if !line.as_ref().ends_with('\n') {
            self.source_code.push('\n');
        }
    }

    /// Check if a source file has been added as dependency (to avoid duplicate emits)
    pub fn has_source_dependency(&self, path: &str) -> bool {
        self.source_dependencies.contains(path)
    }

    /// Mark a source file as dependency
    pub fn add_source_dependency(&mut self, path: impl Into<String>) {
        self.source_dependencies.insert(path.into());
    }

    /// Check if function call for node has been emitted (по рефу ShaderStage::isEmitted).
    pub fn is_function_call_emitted(&self, node_name: &str) -> bool {
        self.emitted_function_calls.contains(node_name)
    }

    /// Mark node's function call as emitted (по рефу ShaderStage::addFunctionCall).
    pub fn add_function_call_emitted(&mut self, node_name: impl Into<String>) {
        self.emitted_function_calls.insert(node_name.into());
    }

    /// Get uniform block by name. Returns None if not found (по рефу getUniformBlock).
    pub fn get_uniform_block(&self, name: &str) -> Option<&VariableBlock> {
        self.uniforms.get(name)
    }

    pub fn get_uniform_block_mut(&mut self, name: &str) -> Option<&mut VariableBlock> {
        self.uniforms.get_mut(name)
    }

    /// Get input block by name.
    pub fn get_input_block(&self, name: &str) -> Option<&VariableBlock> {
        self.inputs.get(name)
    }

    pub fn get_input_block_mut(&mut self, name: &str) -> Option<&mut VariableBlock> {
        self.inputs.get_mut(name)
    }

    /// Get output block by name.
    pub fn get_output_block(&self, name: &str) -> Option<&VariableBlock> {
        self.outputs.get(name)
    }

    pub fn get_output_block_mut(&mut self, name: &str) -> Option<&mut VariableBlock> {
        self.outputs.get_mut(name)
    }

    /// Get constant block.
    pub fn get_constant_block(&self) -> &VariableBlock {
        &self.constants
    }

    pub fn get_constant_block_mut(&mut self) -> &mut VariableBlock {
        &mut self.constants
    }

    /// Get all uniform blocks (for iteration).
    pub fn get_uniform_blocks(&self) -> &HashMap<String, VariableBlock> {
        &self.uniforms
    }

    // ---- Indentation / scope API (matches C++ ShaderStage beginScope/endScope) ----

    /// Write current indentation to source code.
    pub fn begin_line(&mut self) {
        for _ in 0..self.indentation {
            self.source_code.push_str(INDENTATION_STRING);
        }
    }

    /// End the current line, optionally adding a semicolon, then a newline.
    pub fn end_line(&mut self, semicolon: bool) {
        if semicolon {
            self.source_code.push(';');
        }
        self.source_code.push('\n');
    }

    /// Add a newline.
    pub fn new_line(&mut self) {
        self.source_code.push('\n');
    }

    /// Begin a new scope (writes opening bracket, increments indentation).
    pub fn begin_scope(&mut self, punc: ScopePunctuation) {
        let open = match punc {
            ScopePunctuation::CurlyBrackets => "{",
            ScopePunctuation::Parentheses => "(",
            ScopePunctuation::SquareBrackets => "[",
            ScopePunctuation::DoubleSquareBrackets => "[[",
        };
        self.begin_line();
        self.source_code.push_str(open);
        self.source_code.push('\n');
        self.indentation += 1;
        self.scopes.push(punc);
    }

    /// End the current scope (decrements indentation, writes closing bracket).
    /// `semicolon`: append `;` after the closing bracket.
    /// `newline`: append newline after the closing bracket.
    pub fn end_scope(&mut self, semicolon: bool, newline: bool) {
        if let Some(punc) = self.scopes.pop() {
            self.indentation = self.indentation.saturating_sub(1);
            let close = match punc {
                ScopePunctuation::CurlyBrackets => "}",
                ScopePunctuation::Parentheses => ")",
                ScopePunctuation::SquareBrackets => "]",
                ScopePunctuation::DoubleSquareBrackets => "]]",
            };
            self.begin_line();
            self.source_code.push_str(close);
            if semicolon {
                self.source_code.push(';');
            }
            if newline {
                self.source_code.push('\n');
            }
        }
    }

    /// Add an indented line (begin_line + str + end_line). Convenience for `addLine` in C++.
    pub fn add_indent_line(&mut self, line: &str, semicolon: bool) {
        self.begin_line();
        self.source_code.push_str(line);
        self.end_line(semicolon);
    }

    // ---- Function dedup (matches C++ ShaderStage::addFunctionDefinition) ----

    /// Register a function definition by name; returns true if newly added (not a duplicate).
    /// Matches C++ ShaderStage::addFunctionDefinition(const string&).
    pub fn add_function_definition(&mut self, name: impl Into<String>) -> bool {
        self.defined_functions_by_name.insert(name.into())
    }

    /// Check if a function definition has already been registered by name.
    pub fn has_function_definition(&self, name: &str) -> bool {
        self.defined_functions_by_name.contains(name)
    }

    /// Emit a function definition identified by `hash`, if not already emitted in this stage.
    /// Returns true if the definition was emitted (false = was a duplicate).
    pub fn add_function_definition_by_hash(
        &mut self,
        hash: u64,
        emit_fn: impl FnOnce(&mut ShaderStage),
    ) -> bool {
        if self.defined_functions.contains(&hash) {
            return false;
        }
        self.defined_functions.insert(hash);
        emit_fn(self);
        true
    }

    /// Append the Display representation of a value to the code buffer.
    /// Matches C++ ShaderStage::addValue<T> which does: StringStream str; str << value; _code += str.str().
    pub fn add_value<T: std::fmt::Display>(&mut self, value: &T) {
        self.source_code.push_str(&value.to_string());
    }

    // ---- emit_* helpers (mirrors C++ ShaderGenerator::emit* methods on ShaderStage) ----

    /// Append a raw string to the code buffer (matches C++ stage.addString / emitString).
    pub fn emit_string(&mut self, s: &str) {
        self.source_code.push_str(s);
    }

    /// Emit indentation for the current line (matches C++ emitLineBegin / stage.beginLine).
    pub fn emit_line_begin(&mut self) {
        self.begin_line();
    }

    /// Emit line ending, optionally with semicolon then newline (matches C++ emitLineEnd).
    pub fn emit_line_end(&mut self, semicolon: bool) {
        self.end_line(semicolon);
    }

    /// Emit a complete indented line with optional trailing semicolon (matches C++ emitLine).
    /// `semicolon = true` appends ';' before the newline.
    pub fn emit_line(&mut self, line: &str, semicolon: bool) {
        self.begin_line();
        self.source_code.push_str(line);
        self.end_line(semicolon);
    }

    /// Emit a single-line comment: "// comment\n" (matches C++ emitComment / stage.addComment).
    pub fn emit_comment(&mut self, comment: &str) {
        self.begin_line();
        self.source_code.push_str("// ");
        self.source_code.push_str(comment);
        self.source_code.push('\n');
    }

    /// Emit a blank line (matches C++ emitLineBreak / stage.newLine).
    pub fn emit_empty_line(&mut self) {
        self.source_code.push('\n');
    }

    /// Emit a scope-begin bracket with indent increase (matches C++ emitScopeBegin).
    /// Uses CurlyBrackets by default (override via begin_scope with custom ScopePunctuation).
    pub fn emit_scope_begin(&mut self) {
        self.begin_scope(ScopePunctuation::CurlyBrackets);
    }

    /// Emit a scope-end bracket with indent decrease (matches C++ emitScopeEnd).
    pub fn emit_scope_end(&mut self, semicolon: bool, newline: bool) {
        self.end_scope(semicolon, newline);
    }

    /// Emit a simple variable declaration: "[qualifier ]type_name var_name[ = value];\n".
    /// Qualifier is omitted when empty. Value assignment is omitted when None.
    /// This covers the core of C++ emitVariableDeclaration without Syntax dependency.
    pub fn emit_variable_decl(
        &mut self,
        type_name: &str,
        var_name: &str,
        qualifier: &str,
        value: Option<&str>,
    ) {
        self.begin_line();
        if !qualifier.is_empty() {
            self.source_code.push_str(qualifier);
            self.source_code.push(' ');
        }
        self.source_code.push_str(type_name);
        self.source_code.push(' ');
        self.source_code.push_str(var_name);
        if let Some(v) = value {
            self.source_code.push_str(" = ");
            self.source_code.push_str(v);
        }
        self.end_line(true);
    }

    /// Append a pre-loaded source block string to this stage (C++ ShaderStage::addBlock).
    ///
    /// Emits `block_string` as-is to the stage source code and optionally records
    /// `file_path` as a dependency so it is not emitted again (deduplication).
    /// If `file_path` is non-empty and already recorded, the block is skipped.
    pub fn add_block(&mut self, block_string: &str, file_path: &str) {
        // Deduplicate by file path: skip if already included
        if !file_path.is_empty() {
            if self.has_source_dependency(file_path) {
                return;
            }
            self.add_source_dependency(file_path);
        }
        self.source_code.push_str(block_string);
        // Ensure the block ends with a newline so subsequent code starts on its own line
        if !block_string.is_empty() && !block_string.ends_with('\n') {
            self.source_code.push('\n');
        }
    }

    /// Emit source block with recursive `#include` resolution (C++ ShaderStage::addBlock).
    ///
    /// Scans each line for `#include "file"`. When found, resolves the path via
    /// `context.resolve_source_file()`, reads the file, and recursively inlines
    /// its content — matching the C++ addBlock/addInclude pattern from ShaderStage.cpp:314-361.
    pub fn add_block_with_includes(
        &mut self,
        block_string: &str,
        source_filename: &str,
        context: &dyn ShaderImplContext,
    ) {
        let parent_path = if source_filename.is_empty() {
            None
        } else {
            let fp = FilePath::new(source_filename);
            Some(fp.get_parent_path())
        };
        // Apply token substitutions for include filenames (C++ tokenSubstitution)
        let subs = context.get_include_token_substitutions();

        for line in block_string.lines() {
            if let Some(include_file) = extract_include_filename(line) {
                // Apply token substitutions to the include filename
                let mut modified = include_file.to_string();
                for (from, to) in &subs {
                    modified = modified.replace(from, to);
                }
                // Resolve relative to source file's parent directory
                let resolved = context.resolve_source_file(&modified, parent_path.as_ref());
                if let Some(resolved_path) = resolved {
                    // Normalize separators for consistent dedup on Windows
                    let resolved_str = resolved_path.as_str().replace('\\', "/");
                    if !self.includes.contains(&resolved_str) {
                        let content = read_file(&resolved_path);
                        if content.is_empty() {
                            log::error!("Could not find include file: '{}'", include_file);
                            continue;
                        }
                        self.includes.insert(resolved_str.clone());
                        // Recurse into included content
                        self.add_block_with_includes(&content, &resolved_str, context);
                    }
                } else {
                    log::error!("Failed to resolve include file: '{}'", include_file);
                }
            } else {
                self.append_line(line);
            }
        }
    }
}

/// Extract filename from `#include "file"` line (C++ Syntax INCLUDE_STATEMENT + QUOTE).
/// Returns `Some("file")` if the line contains a valid `#include "..."`.
fn extract_include_filename(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if !trimmed.starts_with("#include") {
        return None;
    }
    let after = trimmed["#include".len()..].trim();
    let start = after.find('"')?;
    let rest = &after[start + 1..];
    let end = rest.find('"')?;
    if end == 0 {
        return None;
    }
    Some(&rest[..end])
}

/// Add a shader port to an input block (по рефу addStageInput).
pub fn add_stage_input<'a>(
    block_name: &str,
    type_desc: TypeDesc,
    name: &str,
    stage: &'a mut ShaderStage,
    should_widen: bool,
) -> &'a mut super::shader_node::ShaderPort {
    let inputs = stage.create_input_block(block_name, block_name);
    inputs.add(type_desc, name, None, should_widen)
}

/// Add a shader port to an output block (по рефу addStageOutput).
pub fn add_stage_output<'a>(
    block_name: &str,
    type_desc: TypeDesc,
    name: &str,
    stage: &'a mut ShaderStage,
    should_widen: bool,
) -> &'a mut super::shader_node::ShaderPort {
    let outputs = stage.create_output_block(block_name, block_name);
    outputs.add(type_desc, name, None, should_widen)
}

/// Add a shader port to a uniform block (по рефу addStageUniform).
pub fn add_stage_uniform<'a>(
    block_name: &str,
    type_desc: TypeDesc,
    name: &str,
    stage: &'a mut ShaderStage,
) -> &'a mut super::shader_node::ShaderPort {
    let uniforms = stage.create_uniform_block(block_name, block_name);
    uniforms.add(type_desc, name, None, false)
}

/// Add a uniform with optional value.
pub fn add_stage_uniform_with_value<'a>(
    block_name: &str,
    type_desc: TypeDesc,
    name: &str,
    stage: &'a mut ShaderStage,
    value: Option<Value>,
) -> &'a mut super::shader_node::ShaderPort {
    let uniforms = stage.create_uniform_block(block_name, block_name);
    uniforms.add(type_desc, name, value, false)
}

/// Add a connector variable between two stages (по рефу addStageConnector).
pub fn add_stage_connector(
    block_name: &str,
    instance: &str,
    type_desc: TypeDesc,
    name: &str,
    from_stage: &mut ShaderStage,
    to_stage: &mut ShaderStage,
    should_widen: bool,
) {
    from_stage.create_output_block(block_name, instance);
    to_stage.create_input_block(block_name, instance);
    add_stage_output(
        block_name,
        type_desc.clone(),
        name,
        from_stage,
        should_widen,
    );
    add_stage_input(block_name, type_desc, name, to_stage, should_widen);
}

/// Add a connector block between stages (по рефу addStageConnectorBlock).
pub fn add_stage_connector_block(
    block_name: &str,
    instance: &str,
    from_stage: &mut ShaderStage,
    to_stage: &mut ShaderStage,
) {
    from_stage.create_output_block(block_name, instance);
    to_stage.create_input_block(block_name, instance);
}

/// Shader — contains graph and generated stages/code
#[derive(Debug)]
pub struct Shader {
    pub name: String,
    pub graph: ShaderGraph,
    pub stages: Vec<ShaderStage>,
    pub stage_map: HashMap<String, usize>,
    pub attributes: HashMap<String, Value>,
}

impl Shader {
    /// Create a Shader with no stages (matches C++ Shader constructor).
    /// Generators call `create_stage()` to add the stages they need.
    pub fn new(name: impl Into<String>, graph: ShaderGraph) -> Self {
        Self {
            name: name.into(),
            graph,
            stages: Vec::new(),
            stage_map: HashMap::new(),
            attributes: HashMap::new(),
        }
    }

    /// Create a stage with the given name and return a mutable reference to it.
    /// If a stage with that name already exists, returns the existing one.
    pub fn create_stage(&mut self, name: &str) -> &mut ShaderStage {
        if let Some(&idx) = self.stage_map.get(name) {
            return &mut self.stages[idx];
        }
        let stage = ShaderStage::new(name);
        self.add_stage(stage)
    }

    /// Create HW shader with vertex + pixel stages (vertex first, per MaterialX convention).
    pub fn new_hw(name: impl Into<String>, graph: ShaderGraph) -> Self {
        let name = name.into();
        let mut shader = Self {
            name: name.clone(),
            graph,
            stages: Vec::new(),
            stage_map: HashMap::new(),
            attributes: HashMap::new(),
        };
        let vertex = ShaderStage::new(stage::VERTEX);
        shader.add_stage(vertex);
        let pixel = ShaderStage::new(stage::PIXEL);
        shader.add_stage(pixel);
        shader
    }

    fn add_stage(&mut self, stage: ShaderStage) -> &mut ShaderStage {
        let name = stage.name.clone();
        let idx = self.stages.len();
        self.stage_map.insert(name.clone(), idx);
        self.stages.push(stage);
        &mut self.stages[idx]
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }

    pub fn get_stage(&self, index: usize) -> Option<&ShaderStage> {
        self.stages.get(index)
    }

    pub fn get_stage_mut(&mut self, index: usize) -> Option<&mut ShaderStage> {
        self.stages.get_mut(index)
    }

    pub fn has_stage(&self, name: &str) -> bool {
        self.stage_map.contains_key(name)
    }

    pub fn get_stage_by_name(&self, name: &str) -> Option<&ShaderStage> {
        self.stage_map.get(name).and_then(|&i| self.stages.get(i))
    }

    pub fn get_stage_by_name_mut(&mut self, name: &str) -> Option<&mut ShaderStage> {
        let idx = *self.stage_map.get(name)?;
        self.stages.get_mut(idx)
    }

    pub fn get_graph(&self) -> &ShaderGraph {
        &self.graph
    }

    pub fn get_graph_mut(&mut self) -> &mut ShaderGraph {
        &mut self.graph
    }

    /// Split into graph and stages for non-overlapping borrows during emit.
    pub fn into_parts(self) -> (ShaderGraph, Vec<ShaderStage>) {
        (self.graph, self.stages)
    }

    /// Reconstruct from parts (after emit).
    pub fn from_parts(
        name: impl Into<String>,
        graph: ShaderGraph,
        stages: Vec<ShaderStage>,
    ) -> Self {
        let name = name.into();
        let mut stage_map = HashMap::new();
        for (i, s) in stages.iter().enumerate() {
            stage_map.insert(s.name.clone(), i);
        }
        Self {
            name,
            graph,
            stages,
            stage_map,
            attributes: HashMap::new(),
        }
    }

    pub fn has_classification(&self, c: u32) -> bool {
        self.graph.has_classification(c)
    }

    pub fn set_source_code(&mut self, code: impl Into<String>, stage_name: &str) {
        if let Some(s) = self.get_stage_by_name_mut(stage_name) {
            s.set_source_code(code);
        }
    }

    pub fn get_source_code(&self, stage_name: &str) -> &str {
        self.get_stage_by_name(stage_name)
            .map(|s| s.get_source_code())
            .unwrap_or("")
    }

    pub fn set_attribute(&mut self, name: impl Into<String>, value: Value) {
        self.attributes.insert(name.into(), value);
    }

    pub fn get_attribute(&self, name: &str) -> Option<&Value> {
        self.attributes.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_shader::{TypeDesc, TypeSystem};

    fn float_td() -> TypeDesc {
        TypeSystem::new().get_type("float")
    }

    // -- VariableBlock::find_with_predicate tests --

    #[test]
    fn find_with_predicate_found() {
        let mut block = VariableBlock::new("Uniforms", "");
        block.add(float_td(), "roughness", None, false);
        block.add(float_td(), "metalness", None, false);

        let found = block.find_with_predicate(|p| p.name == "metalness");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "metalness");
    }

    #[test]
    fn find_with_predicate_not_found() {
        let mut block = VariableBlock::new("Uniforms", "");
        block.add(float_td(), "roughness", None, false);

        let found = block.find_with_predicate(|p| p.name == "missing");
        assert!(found.is_none());
    }

    #[test]
    fn find_with_predicate_mut() {
        let mut block = VariableBlock::new("Uniforms", "");
        block.add(float_td(), "alpha", None, false);

        let port = block.find_with_predicate_mut(|p| p.name == "alpha");
        assert!(port.is_some());
    }

    // -- ShaderStage::add_block tests --

    #[test]
    fn add_block_appends_source() {
        let mut stage = ShaderStage::new("pixel");
        stage.add_block("float x = 0.0;", "");
        assert!(stage.get_source_code().contains("float x = 0.0;"));
    }

    #[test]
    fn add_block_deduplicates_by_path() {
        let mut stage = ShaderStage::new("pixel");
        stage.add_block("// block\n", "lib/math.glsl");
        stage.add_block("// block\n", "lib/math.glsl"); // second call: skipped
        // Should appear only once
        let code = stage.get_source_code();
        assert_eq!(code.matches("// block").count(), 1);
    }

    #[test]
    fn add_block_empty_path_always_appends() {
        let mut stage = ShaderStage::new("pixel");
        stage.add_block("line1\n", "");
        stage.add_block("line2\n", ""); // no path = no dedup
        let code = stage.get_source_code();
        assert!(code.contains("line1"));
        assert!(code.contains("line2"));
    }

    #[test]
    fn add_block_ensures_trailing_newline() {
        let mut stage = ShaderStage::new("pixel");
        stage.add_block("no_newline", "");
        assert!(stage.get_source_code().ends_with('\n'));
    }
}
