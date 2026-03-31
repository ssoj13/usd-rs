//! OsoNode — ShaderNodeImpl for compiled .oso shaders (genoslnetwork).
//! По рефу MaterialXGenOsl/Nodes/OsoNode.cpp

use std::path::Path;

use crate::core::ElementPtr;
use crate::core::element::category;
use crate::gen_shader::hash_string;
use crate::gen_shader::{ShaderImplContext, ShaderNodeImpl};

/// ShaderNodeImpl for nodes that reference compiled OSL (.oso) shaders.
/// Implementation's function attr = oso shader name, file attr = directory path.
#[derive(Debug)]
pub struct OsoNode {
    name: String,
    hash: u64,
    oso_name: String,
    oso_path: String,
}

impl OsoNode {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            hash: 0,
            oso_name: String::new(),
            oso_path: String::new(),
        }
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    /// Initialize from genosl Implementation (fallback when no genoslnetwork impl).
    /// oso_name = function, oso_path = parent directory of file (where .oso would be).
    pub fn initialize_from_genosl_fallback(
        &mut self,
        element: &ElementPtr,
        _context: &dyn ShaderImplContext,
    ) {
        let elem = element.borrow();
        if elem.get_category() != category::IMPLEMENTATION {
            panic!("OsoNode::from_genosl: element is not Implementation");
        }
        drop(elem);

        self.name = element.borrow().get_name().to_string();
        self.oso_name = element.borrow().get_attribute_or_empty("function");
        let file_attr = element.borrow().get_attribute_or_empty("file");
        self.oso_path = Path::new(&file_attr)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string());
        if self.oso_path.is_empty() {
            self.oso_path = ".".to_string();
        }
        self.hash = hash_string(&self.oso_name);
    }

    pub fn get_oso_name(&self) -> &str {
        &self.oso_name
    }

    pub fn get_oso_path(&self) -> &str {
        &self.oso_path
    }
}

impl Default for OsoNode {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaderNodeImpl for OsoNode {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &ElementPtr, _context: &dyn ShaderImplContext) {
        let elem = element.borrow();
        if elem.get_category() != category::IMPLEMENTATION {
            panic!(
                "OsoNode: element '{}' is not an Implementation (got category '{}')",
                elem.get_name(),
                elem.get_category()
            );
        }
        drop(elem);

        self.name = element.borrow().get_name().to_string();
        self.oso_name = element.borrow().get_attribute_or_empty("function");
        self.oso_path = element.borrow().get_attribute_or_empty("file");
        self.hash = hash_string(&self.oso_name);
    }

    fn as_oso(&self) -> Option<(&str, &str)> {
        Some((&self.oso_name, &self.oso_path))
    }
}
