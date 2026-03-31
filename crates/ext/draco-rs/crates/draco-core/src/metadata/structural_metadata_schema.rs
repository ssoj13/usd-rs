//! Structural metadata schema (EXT_structural_metadata).
//!
//! What: A minimal JSON-like schema representation for structural metadata.
//! Why: Draco C++ stores the schema as a simple object tree to avoid external
//! JSON dependencies; we mirror that for parity.
//! How: `Object` nodes store a name, type tag, and either children, array items,
//! or a primitive value.
//! Where used: Attached to `StructuralMetadata` and copied with meshes.

/// Schema representation for EXT_structural_metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuralMetadataSchema {
    /// Top-level schema JSON object (must be named "schema").
    pub json: Object,
}

impl StructuralMetadataSchema {
    /// Creates a new schema with the required top-level name.
    pub fn new() -> Self {
        Self {
            json: Object::with_name("schema"),
        }
    }

    /// Returns true if the schema is empty (no child objects).
    pub fn is_empty(&self) -> bool {
        self.json.objects.is_empty()
    }
}

impl Default for StructuralMetadataSchema {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON-like schema object node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    name: String,
    object_type: ObjectType,
    objects: Vec<Object>,
    array: Vec<Object>,
    string: String,
    integer: i32,
    boolean: bool,
}

/// Type tag for schema objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectType {
    Object,
    Array,
    String,
    Integer,
    Boolean,
}

impl Object {
    /// Creates an unnamed OBJECT node.
    pub fn new() -> Self {
        Self::with_name("")
    }

    /// Creates an OBJECT node with a name.
    pub fn with_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            object_type: ObjectType::Object,
            objects: Vec::new(),
            array: Vec::new(),
            string: String::new(),
            integer: 0,
            boolean: false,
        }
    }

    /// Creates a STRING node with a name and value.
    pub fn with_string(name: &str, value: &str) -> Self {
        let mut obj = Self::with_name(name);
        obj.set_string(value);
        obj
    }

    /// Creates an INTEGER node with a name and value.
    pub fn with_integer(name: &str, value: i32) -> Self {
        let mut obj = Self::with_name(name);
        obj.set_integer(value);
        obj
    }

    /// Creates a BOOLEAN node with a name and value.
    pub fn with_boolean(name: &str, value: bool) -> Self {
        let mut obj = Self::with_name(name);
        obj.set_boolean(value);
        obj
    }

    /// Returns the object name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the object type.
    pub fn object_type(&self) -> ObjectType {
        self.object_type
    }

    /// Returns child object list (OBJECT type).
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    /// Returns array elements (ARRAY type).
    pub fn array(&self) -> &[Object] {
        &self.array
    }

    /// Returns string value (STRING type).
    pub fn string(&self) -> &str {
        &self.string
    }

    /// Returns integer value (INTEGER type).
    pub fn integer(&self) -> i32 {
        self.integer
    }

    /// Returns boolean value (BOOLEAN type).
    pub fn boolean(&self) -> bool {
        self.boolean
    }

    /// Looks up a direct child object by name.
    pub fn get_object_by_name(&self, name: &str) -> Option<&Object> {
        self.objects.iter().find(|obj| obj.name == name)
    }

    /// Sets this node to OBJECT type and returns the child list.
    pub fn set_objects(&mut self) -> &mut Vec<Object> {
        self.object_type = ObjectType::Object;
        &mut self.objects
    }

    /// Sets this node to ARRAY type and returns the array list.
    pub fn set_array(&mut self) -> &mut Vec<Object> {
        self.object_type = ObjectType::Array;
        &mut self.array
    }

    /// Sets this node to STRING type with value.
    pub fn set_string(&mut self, value: &str) {
        self.object_type = ObjectType::String;
        self.string = value.to_string();
    }

    /// Sets this node to INTEGER type with value.
    pub fn set_integer(&mut self, value: i32) {
        self.object_type = ObjectType::Integer;
        self.integer = value;
    }

    /// Sets this node to BOOLEAN type with value.
    pub fn set_boolean(&mut self, value: bool) {
        self.object_type = ObjectType::Boolean;
        self.boolean = value;
    }

    /// Deep-copies another object into this one.
    pub fn copy_from(&mut self, src: &Object) {
        self.name = src.name.clone();
        self.object_type = src.object_type;
        self.objects.clear();
        for obj in &src.objects {
            let mut copy = Object::new();
            copy.copy_from(obj);
            self.objects.push(copy);
        }
        self.array.clear();
        for obj in &src.array {
            let mut copy = Object::new();
            copy.copy_from(obj);
            self.array.push(copy);
        }
        self.string = src.string.clone();
        self.integer = src.integer;
        self.boolean = src.boolean;
    }
}

impl Default for Object {
    fn default() -> Self {
        Self::new()
    }
}
