//! Symbol table — scoped symbol management for the compiler.
//!
//! Port of `oscomp_pvt.h` symbol table functionality. Provides hierarchical
//! scope management for variable and function symbol lookups during
//! compilation.

use crate::symbol::Symbol;
use crate::typespec::TypeSpec;
use crate::ustring::UString;
use std::collections::HashMap;

/// A scope level in the symbol table.
#[derive(Debug)]
struct Scope {
    /// Symbols defined in this scope: name -> symbol index.
    symbols: HashMap<UString, usize>,
    /// Parent scope index (None for global scope).
    parent: Option<usize>,
}

/// A function symbol with overload tracking.
#[derive(Debug, Clone)]
pub struct FunctionSymbol {
    pub name: UString,
    pub return_type: TypeSpec,
    pub param_types: Vec<TypeSpec>,
    pub is_builtin: bool,
    pub symbol_index: usize,
    /// Encoded argument type codes for polymorphic dispatch (C++ symtab.h:94 m_argcodes).
    pub argcodes: Option<UString>,
}

/// The symbol table.
#[derive(Debug)]
pub struct SymbolTable {
    /// All symbols (flat vector, indexed by symbol ID).
    pub symbols: Vec<Symbol>,
    /// Scopes.
    scopes: Vec<Scope>,
    /// Current scope index.
    current_scope: usize,
    /// Function overloads: name -> list of function symbols.
    functions: HashMap<UString, Vec<FunctionSymbol>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        let global = Scope {
            symbols: HashMap::new(),
            parent: None,
        };
        Self {
            symbols: Vec::new(),
            scopes: vec![global],
            current_scope: 0,
            functions: HashMap::new(),
        }
    }

    /// Enter a new scope.
    pub fn push_scope(&mut self) {
        let new_scope = Scope {
            symbols: HashMap::new(),
            parent: Some(self.current_scope),
        };
        self.current_scope = self.scopes.len();
        self.scopes.push(new_scope);
    }

    /// Leave the current scope.
    pub fn pop_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current_scope].parent {
            self.current_scope = parent;
        }
    }

    /// Insert a symbol into the current scope. Returns the symbol index.
    pub fn insert(&mut self, sym: Symbol) -> usize {
        let idx = self.symbols.len();
        let name = sym.name;
        self.symbols.push(sym);
        self.scopes[self.current_scope].symbols.insert(name, idx);
        idx
    }

    /// Look up a symbol by name, searching from current scope up to global.
    pub fn lookup(&self, name: UString) -> Option<usize> {
        let mut scope_idx = self.current_scope;
        loop {
            if let Some(&idx) = self.scopes[scope_idx].symbols.get(&name) {
                return Some(idx);
            }
            match self.scopes[scope_idx].parent {
                Some(parent) => scope_idx = parent,
                None => return None,
            }
        }
    }

    /// Look up a symbol only in the current scope (no parent search).
    pub fn lookup_local(&self, name: UString) -> Option<usize> {
        self.scopes[self.current_scope].symbols.get(&name).copied()
    }

    /// Get a symbol by index.
    pub fn get(&self, idx: usize) -> Option<&Symbol> {
        self.symbols.get(idx)
    }

    /// Get a mutable symbol by index.
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut Symbol> {
        self.symbols.get_mut(idx)
    }

    /// Register a function (potentially overloaded).
    pub fn register_function(&mut self, func: FunctionSymbol) {
        self.functions.entry(func.name).or_default().push(func);
    }

    /// Look up function overloads by name.
    pub fn lookup_function(&self, name: UString) -> &[FunctionSymbol] {
        self.functions
            .get(&name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Find the best function overload given argument types.
    pub fn resolve_function(
        &self,
        name: UString,
        arg_types: &[TypeSpec],
    ) -> Option<&FunctionSymbol> {
        let overloads = self.lookup_function(name);
        if overloads.is_empty() {
            return None;
        }

        // Exact match first
        for f in overloads {
            if f.param_types.len() == arg_types.len() {
                let exact = f.param_types.iter().zip(arg_types).all(|(a, b)| a == b);
                if exact {
                    return Some(f);
                }
            }
        }

        // Compatible match (with implicit conversions)
        for f in overloads {
            if f.param_types.len() == arg_types.len() {
                let compatible = f.param_types.iter().zip(arg_types).all(|(formal, actual)| {
                    crate::typecheck::TypeChecker::assignable(*formal, *actual)
                });
                if compatible {
                    return Some(f);
                }
            }
        }

        // Variadic: match by prefix (for printf, format, etc.)
        for f in overloads {
            if !f.param_types.is_empty() && arg_types.len() >= f.param_types.len() {
                return Some(f);
            }
        }

        None
    }

    /// Current scope depth (0 = global).
    pub fn scope_depth(&self) -> usize {
        let mut depth = 0;
        let mut s = self.current_scope;
        while let Some(parent) = self.scopes[s].parent {
            depth += 1;
            s = parent;
        }
        depth
    }

    /// Total number of symbols.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Is the symbol table empty?
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymType;
    use crate::typedesc::TypeDesc;

    fn make_sym(name: &str, stype: SymType) -> Symbol {
        let s = Symbol::new(
            UString::new(name),
            TypeSpec::from_simple(TypeDesc::FLOAT),
            stype,
        );
        s
    }

    #[test]
    fn test_basic_insert_lookup() {
        let mut st = SymbolTable::new();
        let idx = st.insert(make_sym("x", SymType::Local));
        assert_eq!(st.lookup(UString::new("x")), Some(idx));
        assert_eq!(st.lookup(UString::new("y")), None);
    }

    #[test]
    fn test_scope_hierarchy() {
        let mut st = SymbolTable::new();
        st.insert(make_sym("global_var", SymType::Global));

        st.push_scope();
        st.insert(make_sym("local_var", SymType::Local));
        // Can see both local and global
        assert!(st.lookup(UString::new("global_var")).is_some());
        assert!(st.lookup(UString::new("local_var")).is_some());

        st.pop_scope();
        // Global still visible, local not
        assert!(st.lookup(UString::new("global_var")).is_some());
        assert!(st.lookup(UString::new("local_var")).is_none());
    }

    #[test]
    fn test_scope_shadowing() {
        let mut st = SymbolTable::new();
        let idx1 = st.insert(make_sym("x", SymType::Global));

        st.push_scope();
        let idx2 = st.insert(make_sym("x", SymType::Local));

        // Inner x shadows outer x
        assert_eq!(st.lookup(UString::new("x")), Some(idx2));

        st.pop_scope();
        assert_eq!(st.lookup(UString::new("x")), Some(idx1));
    }

    #[test]
    fn test_function_registration() {
        let mut st = SymbolTable::new();
        let name = UString::new("sin");
        st.register_function(FunctionSymbol {
            name,
            return_type: TypeSpec::from_simple(TypeDesc::FLOAT),
            param_types: vec![TypeSpec::from_simple(TypeDesc::FLOAT)],
            is_builtin: true,
            symbol_index: 0,
            argcodes: None,
        });

        let overloads = st.lookup_function(name);
        assert_eq!(overloads.len(), 1);
    }

    #[test]
    fn test_scope_depth() {
        let mut st = SymbolTable::new();
        assert_eq!(st.scope_depth(), 0);
        st.push_scope();
        assert_eq!(st.scope_depth(), 1);
        st.push_scope();
        assert_eq!(st.scope_depth(), 2);
        st.pop_scope();
        assert_eq!(st.scope_depth(), 1);
    }
}
