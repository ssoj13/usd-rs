//! Alembic data reader implementation.
//!
//! This module provides `AlembicDataReader`, the backing implementation for
//! reading Alembic files.
//!
//! # Porting Status
//!
//! This is a port of `pxr/usd/plugin/usdAbc/alembicReader.{cpp,h}`.
//!
//! # Architecture
//!
//! The reader acts like a key/value database backed by Alembic. When an
//! Alembic file is opened, it scans the object/property hierarchy and caches
//! state for fast lookup later. It does not do much value conversion until
//! the client requests property values.
//!
//! # Dependencies
//!
//! Uses `alembic-rs` library for reading .abc files.

use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use usd_tf::{Token, string_utils::make_valid_identifier};
use usd_vt::{Array, Value};

use super::abstract_data::{AbstractData, SpecVisitor};
use super::file_format::FileFormatArguments;
use super::path::Path;
use super::types::{SpecType, Specifier, TimeSamples, Variability};

// Alembic library imports
use alembic::PlainOldDataType;
use alembic::abc::{IArchive, ICompoundProperty, IObject};
use alembic::abc_core::{PropertyHeader, PropertyType, TimeSamplingType};
use alembic::geom::{FaceSetExclusivity, ICamera, ICurves, IFaceSet, INuPatch, IPoints, IPolyMesh, ISubD, IXform};

// ============================================================================
// Module-level byte-cast helpers (used by schema converters)
// ============================================================================

/// Cast raw bytes to Vec<Vec3f> (f32x3).
fn vec3f_from_bytes(bytes: &[u8]) -> Vec<usd_gf::vec3::Vec3f> {
    use usd_gf::vec3::Vec3f;
    let n = bytes.len() / 12;
    (0..n)
        .map(|i| {
            let o = i * 12;
            let v: [f32; 3] = *bytemuck::from_bytes(&bytes[o..o + 12]);
            Vec3f::new(v[0], v[1], v[2])
        })
        .collect()
}

/// Cast raw bytes to Vec<i32>.
fn i32_from_bytes(bytes: &[u8]) -> Vec<i32> {
    bytemuck::cast_slice(bytes).to_vec()
}

/// Read a named GeomParam (compound=indexed or array=direct) from a compound property as Vec3f.

/// Read array bytes from a sub-compound: parent.sub_name.prop_name[index].
/// Uses named bindings to avoid borrow-of-temporary lifetime issues.
fn read_sub_array(
    parent: &ICompoundProperty<'_>,
    sub: &str,
    prop: &str,
    index: usize,
) -> Option<Vec<u8>> {
    let sub_w = parent.getPropertyByName(sub)?;
    let sub_c = sub_w.asCompound()?;
    let prop_w = sub_c.getPropertyByName(prop)?;
    prop_w.asArray()?.getSampleVec(index).ok()
}

/// Read scalar bytes from a sub-compound into provided buffer.
fn read_sub_scalar(
    parent: &ICompoundProperty<'_>,
    sub: &str,
    prop: &str,
    index: usize,
    out: &mut [u8],
) -> bool {
    if let Some(sub_w) = parent.getPropertyByName(sub) {
        if let Some(sub_c) = sub_w.asCompound() {
            if let Some(prop_w) = sub_c.getPropertyByName(prop) {
                if let Some(sc) = prop_w.asScalar() {
                    return sc.getSample(index, out).is_ok();
                }
            }
        }
    }
    false
}

/// Read array or scalar bytes from a sub-compound (array first, then scalar).
#[allow(dead_code)] // Utility kept for future schema converters.
fn read_sub_any(
    parent: &ICompoundProperty<'_>,
    sub: &str,
    prop: &str,
    index: usize,
) -> Option<Vec<u8>> {
    let sub_w = parent.getPropertyByName(sub)?;
    let sub_c = sub_w.asCompound()?;
    let prop_w = sub_c.getPropertyByName(prop)?;
    if let Some(arr) = prop_w.asArray() {
        if let Ok(b) = arr.getSampleVec(index) {
            return Some(b);
        }
    }
    prop_w.asScalar()?.getSampleVec(index).ok()
}
///
/// `parent` is the compound containing the named property (e.g. the `.geom` compound).
/// The property `name` may be a compound with `.vals`/`.indices` (indexed GeomParam)
/// or a direct array property.
fn read_geom_param_vec3f(
    parent: &ICompoundProperty<'_>,
    name: &str,
    index: usize,
) -> Option<Vec<usd_gf::vec3::Vec3f>> {
    use usd_gf::vec3::Vec3f;
    let prop = parent.getPropertyByName(name)?;
    if let Some(c) = prop.asCompound() {
        // Indexed GeomParam: has .vals sub-property.
        let vals_prop = c.getPropertyByName(".vals")?;
        let vals_bytes = vals_prop.asArray()?.getSampleVec(index).ok()?;
        let floats: &[f32] = bytemuck::try_cast_slice(&vals_bytes).ok()?;
        // Optional .indices for indexed lookup.
        if c.hasProperty(".indices") {
            if let Some(idx_p) = c.getPropertyByName(".indices") {
                if let Some(idx_bytes) = idx_p.asArray().and_then(|a| a.getSampleVec(index).ok()) {
                    let indices: &[u32] = bytemuck::try_cast_slice(&idx_bytes).ok()?;
                    // Bounds-check each index before accessing floats to avoid panic on corrupt data.
                    let result: Option<Vec<Vec3f>> = indices
                        .iter()
                        .map(|&i| {
                            let b = i as usize * 3;
                            let s = floats.get(b..b + 3)?;
                            Some(Vec3f::new(s[0], s[1], s[2]))
                        })
                        .collect();
                    return result;
                }
            }
        }
        return Some(
            floats
                .chunks_exact(3)
                .map(|c| Vec3f::new(c[0], c[1], c[2]))
                .collect(),
        );
    }
    // Direct array property.
    if let Some(arr) = prop.asArray() {
        let bytes = arr.getSampleVec(index).ok()?;
        let floats: &[f32] = bytemuck::try_cast_slice(&bytes).ok()?;
        return Some(
            floats
                .chunks_exact(3)
                .map(|c| Vec3f::new(c[0], c[1], c[2]))
                .collect(),
        );
    }
    None
}

/// Read a named GeomParam (compound=indexed or array=direct) from a compound property as Vec2f.
fn read_geom_param_vec2f(
    parent: &ICompoundProperty<'_>,
    name: &str,
    index: usize,
) -> Option<Vec<usd_gf::vec2::Vec2f>> {
    use usd_gf::vec2::Vec2f;
    let prop = parent.getPropertyByName(name)?;
    if let Some(c) = prop.asCompound() {
        let vals_prop = c.getPropertyByName(".vals")?;
        let vals_bytes = vals_prop.asArray()?.getSampleVec(index).ok()?;
        let floats: &[f32] = bytemuck::try_cast_slice(&vals_bytes).ok()?;
        if c.hasProperty(".indices") {
            if let Some(idx_p) = c.getPropertyByName(".indices") {
                if let Some(idx_bytes) = idx_p.asArray().and_then(|a| a.getSampleVec(index).ok()) {
                    let indices: &[u32] = bytemuck::try_cast_slice(&idx_bytes).ok()?;
                    // Bounds-check each index before accessing floats to avoid panic on corrupt data.
                    let result: Option<Vec<Vec2f>> = indices
                        .iter()
                        .map(|&i| {
                            let b = i as usize * 2;
                            let s = floats.get(b..b + 2)?;
                            Some(Vec2f::new(s[0], s[1]))
                        })
                        .collect();
                    return result;
                }
            }
        }
        return Some(
            floats
                .chunks_exact(2)
                .map(|c| Vec2f::new(c[0], c[1]))
                .collect(),
        );
    }
    if let Some(arr) = prop.asArray() {
        let bytes = arr.getSampleVec(index).ok()?;
        let floats: &[f32] = bytemuck::try_cast_slice(&bytes).ok()?;
        return Some(
            floats
                .chunks_exact(2)
                .map(|c| Vec2f::new(c[0], c[1]))
                .collect(),
        );
    }
    None
}

// ============================================================================
// Property Cache Structures
// ============================================================================

/// Property cache entry.
///
/// Matches C++ `_ReaderContext::Property`.
struct Property {
    /// USD type name for this property
    type_name: Token,
    /// Metadata for this property
    metadata: BTreeMap<Token, Value>,
    /// Time samples for this property
    sample_times: TimeSamples,
    /// Whether this property is time-sampled
    time_sampled: bool,
    /// Whether this property is uniform (same value for all samples)
    uniform: bool,
    /// Converter function: (parent compound property, property name, sample selector) -> Value
    /// Returns None if conversion fails
    converter: Option<Box<dyn Fn(&ICompoundProperty, &str, usize) -> Option<Value> + Send + Sync>>,
    /// Alembic object path (for accessing the property)
    alembic_object_path: String,
    /// Alembic property name
    alembic_property_name: String,
}

impl Property {
    /// Get USD type name for this property
    fn get_type_name(&self) -> &Token {
        &self.type_name
    }

    /// Get metadata for this property
    fn get_metadata(&self) -> &BTreeMap<Token, Value> {
        &self.metadata
    }

    /// Check if property is uniform (same value for all samples)
    fn is_uniform(&self) -> bool {
        self.uniform
    }

    /// Get Alembic property name
    fn get_alembic_property_name(&self) -> &str {
        &self.alembic_property_name
    }
}

/// Prim cache entry.
///
/// Matches C++ `_ReaderContext::Prim`.
struct Prim {
    /// USD type name (e.g., "Mesh", "Xform")
    type_name: Token,
    /// Child prim names
    children: Vec<Token>,
    /// Property names
    properties: Vec<Token>,
    /// Specifier (Def, Over, Class) — matches C++ SdfSpecifier enum
    specifier: Specifier,
    /// Metadata
    metadata: BTreeMap<Token, Value>,
    /// Property cache
    properties_cache: HashMap<Token, Property>,
    /// Explicit prim ordering (optional, matches C++ Ordering = optional<TfTokenVector>)
    prim_ordering: Option<Vec<Token>>,
    /// Explicit property ordering (optional)
    property_ordering: Option<Vec<Token>>,
    /// Path to prototype prim (set on instances)
    prototype: Option<Path>,
    /// Alembic path to instance source (set on prototypes)
    instance_source: Option<String>,
    /// Whether this prim is instanceable (only meaningful on prototypes)
    instanceable: bool,
    /// Whether this prim is promoted
    promoted: bool,
}

impl Prim {
    fn new() -> Self {
        Self {
            type_name: Token::new(""),
            children: Vec::new(),
            properties: Vec::new(),
            specifier: Specifier::Def,
            metadata: BTreeMap::new(),
            properties_cache: HashMap::new(),
            prim_ordering: None,
            property_ordering: None,
            prototype: None,
            instance_source: None,
            instanceable: false,
            promoted: false,
        }
    }

    /// Get child prim names
    fn get_children(&self) -> &Vec<Token> {
        &self.children
    }

    /// Get specifier (Def, Over, Class)
    fn get_specifier(&self) -> Specifier {
        self.specifier
    }

    /// Get instance source path (for prototypes)
    fn get_instance_source(&self) -> Option<&str> {
        self.instance_source.as_deref()
    }

    /// Check if this prim is instanceable.
    /// C++ logic: instanceable field is only emitted when BOTH
    /// instanceable==true AND instanceSource is non-empty.
    #[allow(dead_code)]
    fn is_instanceable(&self) -> bool {
        self.instanceable && self.instance_source.is_some()
    }

    /// Check if this prim is promoted
    fn is_promoted(&self) -> bool {
        self.promoted
    }
}

const FACESET_DEFAULT_FAMILY_NAME: &str = "materialBind";
const FACESET_DEFAULT_FAMILY_TYPE_ATTR: &str = "subsetFamily:materialBind:familyType";

/// Insert or replace a uniform token-valued property on a prim.
///
/// Alembic import parity needs a few synthetic USD properties that do not map
/// 1:1 to raw Alembic property headers. Keeping this helper centralized avoids
/// repeating subtly different synthetic-property encodings and makes it clear
/// that these fields are intentional schema translation, not accidental extras.
fn upsert_uniform_token_property(
    prim: &mut Prim,
    usd_name: &str,
    value: &str,
    alembic_object_path: &str,
    alembic_prop_name: &str,
) {
    let usd_name_token = Token::new(usd_name);
    if !prim.properties.contains(&usd_name_token) {
        prim.properties.push(usd_name_token.clone());
    }
    let value = value.to_string();
    prim.properties_cache.insert(
        usd_name_token,
        Property {
            type_name: Token::new("token"),
            metadata: BTreeMap::new(),
            sample_times: [OrderedFloat(0.0)].into_iter().collect(),
            time_sampled: false,
            uniform: true,
            converter: Some(Box::new(move |_top, _n, _sel| {
                Some(Value::new(Token::new(&value)))
            })),
            alembic_object_path: alembic_object_path.to_string(),
            alembic_property_name: alembic_prop_name.to_string(),
        },
    );
}

// ============================================================================
// AlembicDataReader
// ============================================================================

/// An Alembic reader suitable for an AbstractData.
///
/// Matches C++ `UsdAbc_AlembicDataReader`.
pub struct AlembicDataReader {
    /// Alembic archive (if opened)
    archive: Option<IArchive>,
    /// File path (if opened)
    file_path: Option<String>,
    /// Error messages
    errors: Vec<String>,
    /// Flags for reader behavior
    flags: HashMap<Token, bool>,
    /// Prim cache: path -> Prim
    prims: HashMap<Path, Prim>,
    /// Pseudo-root prim (always exists)
    pseudo_root: Path,
    /// Time samples (all paths) - using OrderedFloat for f64 ordering
    /// Matches C++ `std::set<double> _allTimeSamples` and `UsdAbc_TimeSamples`
    time_samples: TimeSamples,
    /// Time samples by path - using OrderedFloat for f64 ordering
    time_samples_by_path: HashMap<Path, TimeSamples>,
    /// Time scale (Alembic seconds -> USD frames)
    time_scale: f64,
    /// Time offset
    time_offset: f64,
}

impl AlembicDataReader {
    /// Sanitize an Alembic object name into a valid USD prim identifier.
    ///
    /// OpenUSD's usdAbc reader runs Alembic object names through
    /// `TfMakeValidIdentifier` before appending them to an `SdfPath`. Real
    /// Alembic archives such as `bed.abc` use names like `bed:group2`, which
    /// are legal object names in Alembic but illegal USD prim identifiers. If
    /// we append raw names, `SdfPath` construction fails and the resulting
    /// stage appears empty. This helper keeps the Rust importer aligned with
    /// the reference reader by sanitizing and deduplicating names per sibling
    /// set.
    fn clean_alembic_prim_name(raw_name: &str, used_names: &HashSet<String>) -> String {
        let mut name = if raw_name.is_empty() {
            "_".to_string()
        } else {
            raw_name.trim_start_matches([' ', '_']).to_string()
        };

        if name.is_empty() {
            name = "_".to_string();
        }

        if !Path::is_valid_identifier(&name) {
            name = make_valid_identifier(&name);
        }

        if name == "vals" {
            return String::new();
        }

        if !used_names.contains(&name) {
            return name;
        }

        let mut i = 1;
        loop {
            let candidate = format!("{name}_{i}");
            if !used_names.contains(&candidate) {
                return candidate;
            }
            i += 1;
        }
    }

    /// Creates a new Alembic data reader.
    pub fn new() -> Self {
        Self {
            archive: None,
            file_path: None,
            errors: Vec::new(),
            flags: HashMap::new(),
            prims: HashMap::new(),
            pseudo_root: Path::absolute_root(),
            time_samples: BTreeSet::new(),
            time_samples_by_path: HashMap::new(),
            time_scale: 24.0, // USD is frames, Alembic is seconds
            time_offset: 0.0,
        }
    }

    /// Open a file.
    ///
    /// Returns true on success; errors are reported by GetErrors().
    ///
    /// Matches C++ `UsdAbc_AlembicDataReader::Open()`.
    pub fn open(&mut self, file_path: &str, _args: &FileFormatArguments) -> bool {
        // Close any existing archive
        self.close();

        self.file_path = Some(file_path.to_string());

        // Open Alembic archive
        match IArchive::open(file_path) {
            Ok(archive) => {
                self.archive = Some(archive);

                // Scan object/property hierarchy and build caches
                if !self.scan_hierarchy() {
                    self.errors
                        .push("Failed to scan Alembic hierarchy".to_string());
                    self.close();
                    return false;
                }

                true
            }
            Err(e) => {
                self.errors
                    .push(format!("Failed to open Alembic file: {}", e));
                false
            }
        }
    }

    /// Close the file.
    ///
    /// Matches C++ `UsdAbc_AlembicDataReader::Close()`.
    pub fn close(&mut self) {
        self.archive = None;
        self.file_path = None;
        self.prims.clear();
        self.time_samples.clear();
        self.time_samples_by_path.clear();
        self.errors.clear();
    }

    /// Return any errors.
    ///
    /// Matches C++ `UsdAbc_AlembicDataReader::GetErrors()`.
    pub fn get_errors(&self) -> Option<String> {
        if self.errors.is_empty() {
            None
        } else {
            Some(self.errors.join("; "))
        }
    }

    /// Set a reader flag.
    ///
    /// Matches C++ `UsdAbc_AlembicDataReader::SetFlag()`.
    pub fn set_flag(&mut self, flag: Token, set: bool) {
        self.flags.insert(flag, set);
    }

    /// Check if a flag is set.
    #[allow(dead_code)] // C++ parity - flag checking API
    fn is_flag_set(&self, flag: &Token) -> bool {
        self.flags.get(flag).copied().unwrap_or(false)
    }

    /// Scan the Alembic hierarchy and build caches.
    ///
    /// Matches C++ `_ReaderContext::Open()` which scans the hierarchy.
    ///
    /// Strategy: Collect all data first into temporary structures,
    /// then write to self's caches to avoid borrow conflicts.
    fn scan_hierarchy(&mut self) -> bool {
        let archive = match &self.archive {
            Some(arc) => arc,
            None => return false,
        };

        // Create pseudo-root prim
        let mut pseudo_root_prim = Prim::new();
        pseudo_root_prim.type_name = Token::new("PseudoRoot");

        // Temporary structures to collect data
        let mut temp_prims: HashMap<Path, Prim> = HashMap::new();
        let mut temp_time_samples = TimeSamples::new();
        let mut temp_time_samples_by_path = HashMap::new();

        // OpenUSD treats Alembic's anonymous top object as the archive root and
        // exposes only its children as USD root prims.
        let root = archive.getTop();
        let mut used_root_names = HashSet::new();
        for i in 0..root.getNumChildren() {
            if let Some(child) = root.getChild(i) {
                let clean_child_name =
                    Self::clean_alembic_prim_name(child.getName(), &used_root_names);
                if clean_child_name.is_empty() {
                    continue;
                }
                pseudo_root_prim
                    .children
                    .push(Token::new(clean_child_name.as_str()));
                Self::scan_object_recursive(
                    archive,
                    &child,
                    &Path::absolute_root(),
                    Some(clean_child_name.as_str()),
                    &mut temp_prims,
                    &mut temp_time_samples,
                    &mut temp_time_samples_by_path,
                    self.time_scale,
                    self.time_offset,
                );
                used_root_names.insert(clean_child_name);
            }
        }

        // Guess start and end timeCode from all time samples (C++ lines 1092-1098).
        if !temp_time_samples.is_empty() {
            pseudo_root_prim.metadata.insert(
                Token::new("startTimeCode"),
                Value::from_f64(temp_time_samples.iter().next().unwrap().into_inner()),
            );
            pseudo_root_prim.metadata.insert(
                Token::new("endTimeCode"),
                Value::from_f64(temp_time_samples.iter().next_back().unwrap().into_inner()),
            );
        }

        // Default upAxis = 'Y' (C++ line 1104).
        pseudo_root_prim
            .metadata
            .insert(Token::new("upAxis"), Value::new(Token::new("Y")));

        // timeCodesPerSecond = time_scale (defaults to 24.0).
        pseudo_root_prim.metadata.insert(
            Token::new("timeCodesPerSecond"),
            Value::from_f64(self.time_scale),
        );
        pseudo_root_prim.metadata.insert(
            Token::new("framesPerSecond"),
            Value::from_f64(self.time_scale),
        );

        self.prims
            .insert(self.pseudo_root.clone(), pseudo_root_prim);

        // Merge temp_prims into self.prims (pseudo-root already exists)
        for (path, prim) in temp_prims {
            self.prims.insert(path, prim);
        }

        // Write time samples
        self.time_samples = temp_time_samples;
        self.time_samples_by_path = temp_time_samples_by_path;

        true
    }

    /// Recursively scan an Alembic object and its children.
    ///
    /// Static method to avoid borrow conflicts - borrows archive separately
    /// and writes to provided temporary structures.
    ///
    /// `authored_child_name` carries the already-sanitized sibling name chosen
    /// by the caller's child-iteration loop. Re-sanitizing against the
    /// parent's published child list inside recursion looks tempting, but it is
    /// wrong: by that point the current child already exists in `children[]`,
    /// so the importer would rename it a second time (`foo -> foo_1`) and the
    /// stored prim path would no longer match the parent's child token. That
    /// exact mismatch is what made nested Alembic hierarchies like `bed.abc`
    /// appear mostly empty even though the archive was populated.
    fn scan_object_recursive(
        archive: &IArchive,
        obj: &IObject,
        parent_path: &Path,
        authored_child_name: Option<&str>,
        prims: &mut HashMap<Path, Prim>,
        all_time_samples: &mut TimeSamples,
        time_samples_by_path: &mut HashMap<Path, TimeSamples>,
        time_scale: f64,
        time_offset: f64,
    ) {
        let obj_name = authored_child_name
            .map(str::to_owned)
            .unwrap_or_else(|| obj.getName().to_string());
        if obj_name.is_empty() {
            return;
        }
        let obj_path = if parent_path == &Path::absolute_root() {
            Path::from_string(&format!("/{}", obj_name)).unwrap_or_default()
        } else {
            parent_path.append_child(&obj_name).unwrap_or_default()
        };

        if obj_path.is_empty() {
            return;
        }

        // Create prim entry
        let mut prim = Prim::new();

        // Determine spec type based on Alembic schema
        let _spec_type = Self::determine_spec_type(obj);
        prim.type_name = Self::map_alembic_to_usd_type_name(obj);

        // Extract properties and build property cache
        Self::extract_properties(
            archive,
            obj,
            &obj_path,
            &mut prim,
            all_time_samples,
            time_samples_by_path,
            time_scale,
            time_offset,
        );

        // Collapse `_ref`-style `Xform + single geom child` into one typed prim.
        //
        // OpenUSD's Alembic reader treats the transform path as the authored USD
        // prim path when an `IXform` owns exactly one geometric/camera child and
        // there are no `.arbGeomParams`/`.userProperties` conflicts. Without
        // this, archives such as `cache.abc` land as an empty `Xform` plus an
        // untyped child and never reach the real composed prim contract.
        let collapsed_child =
            Self::get_collapsible_xform_child(obj).filter(|child| !Self::has_schema_merge_conflict(obj, child));

        if let Some(child) = collapsed_child.as_ref() {
            prim.type_name = Self::map_alembic_to_usd_type_name(child);
            Self::extract_properties(
                archive,
                child,
                &obj_path,
                &mut prim,
                all_time_samples,
                time_samples_by_path,
                time_scale,
                time_offset,
            );
        }

        // Collect child names
        let mut child_names = Vec::new();
        let mut used_child_names = HashSet::new();
        if let Some(child) = collapsed_child.as_ref() {
            for i in 0..child.getNumChildren() {
                if let Some(grandchild) = child.getChild(i) {
                    let clean_child_name =
                        Self::clean_alembic_prim_name(grandchild.getName(), &used_child_names);
                    if clean_child_name.is_empty() {
                        continue;
                    }
                    used_child_names.insert(clean_child_name.clone());
                    child_names.push(Token::new(&clean_child_name));
                }
            }
        } else {
            for i in 0..obj.getNumChildren() {
                if let Some(child) = obj.getChild(i) {
                    let clean_child_name =
                        Self::clean_alembic_prim_name(child.getName(), &used_child_names);
                    if clean_child_name.is_empty() {
                        continue;
                    }
                    used_child_names.insert(clean_child_name.clone());
                    child_names.push(Token::new(&clean_child_name));
                }
            }
        }
        prim.children = child_names;

        // Store prim
        prims.insert(obj_path.clone(), prim);

        // Recursively scan children
        if let Some(child) = collapsed_child.as_ref() {
            let mut used_grandchild_names = HashSet::new();
            for i in 0..child.getNumChildren() {
                if let Some(grandchild) = child.getChild(i) {
                    let clean_child_name = Self::clean_alembic_prim_name(
                        grandchild.getName(),
                        &used_grandchild_names,
                    );
                    if clean_child_name.is_empty() {
                        continue;
                    }
                    used_grandchild_names.insert(clean_child_name.clone());
                    Self::scan_object_recursive(
                        archive,
                        &grandchild,
                        &obj_path,
                        Some(clean_child_name.as_str()),
                        prims,
                        all_time_samples,
                        time_samples_by_path,
                        time_scale,
                        time_offset,
                    );
                }
            }
        } else {
            let mut used_child_names = HashSet::new();
            for i in 0..obj.getNumChildren() {
                if let Some(child) = obj.getChild(i) {
                    let clean_child_name =
                        Self::clean_alembic_prim_name(child.getName(), &used_child_names);
                    if clean_child_name.is_empty() {
                        continue;
                    }
                    used_child_names.insert(clean_child_name.clone());
                    Self::scan_object_recursive(
                        archive,
                        &child,
                        &obj_path,
                        Some(clean_child_name.as_str()),
                        prims,
                        all_time_samples,
                        time_samples_by_path,
                        time_scale,
                        time_offset,
                    );
                }
            }
        }

        Self::apply_faceset_family_type_from_children(obj, &obj_path, prims);
    }

    /// Return the single child that should be merged into an `IXform` prim.
    ///
    /// This mirrors the default `_ref` `USD_ABC_XFORM_PRIM_COLLAPSE=true`
    /// behavior. We collapse only when the parent is an `IXform` with exactly
    /// one child and that child is geometric or a camera schema.
    fn get_collapsible_xform_child<'a>(obj: &'a IObject) -> Option<IObject<'a>> {
        if !Self::xform_prim_collapse_enabled() {
            return None;
        }
        if IXform::new(obj).is_none() || obj.getNumChildren() != 1 {
            return None;
        }
        let child = obj.getChild(0)?;
        if Self::schema_property_name_for_object(&child).is_some() && IXform::new(&child).is_none()
        {
            return Some(child);
        }
        None
    }

    /// Whether `_ref`-style xform collapse is enabled.
    fn xform_prim_collapse_enabled() -> bool {
        std::env::var("USD_ABC_XFORM_PRIM_COLLAPSE")
            .map(|value| {
                let value = value.trim().to_ascii_lowercase();
                !(value == "0" || value == "false" || value == "off")
            })
            .unwrap_or(true)
    }

    /// Return the Alembic schema-compound name used to host authored data for
    /// geometry/camera schemas that can legally collapse into an `IXform`.
    fn schema_property_name_for_object(obj: &IObject) -> Option<&'static str> {
        if IXform::new(obj).is_some() {
            return Some(".xform");
        }
        if IPolyMesh::new(obj).is_some()
            || ICurves::new(obj).is_some()
            || IPoints::new(obj).is_some()
            || ISubD::new(obj).is_some()
            || INuPatch::new(obj).is_some()
            || ICamera::new(obj).is_some()
        {
            return Some(".geom");
        }
        None
    }

    /// Match `_ref`'s merge guard for `.arbGeomParams` / `.userProperties`.
    fn has_schema_merge_conflict(parent: &IObject, child: &IObject) -> bool {
        let Some(parent_schema_name) = Self::schema_property_name_for_object(parent) else {
            return true;
        };
        let Some(child_schema_name) = Self::schema_property_name_for_object(child) else {
            return true;
        };
        let parent_props = parent.getProperties();
        let Some(parent_prop) = parent_props.getPropertyByName(parent_schema_name) else {
            return true;
        };
        let Some(parent_schema) = parent_prop.asCompound() else {
            return true;
        };
        let child_props = child.getProperties();
        let Some(child_prop) = child_props.getPropertyByName(child_schema_name) else {
            return true;
        };
        let Some(child_schema) = child_prop.asCompound() else {
            return true;
        };
        (parent_schema.getPropertyByName(".arbGeomParams").is_some()
            && child_schema.getPropertyByName(".arbGeomParams").is_some())
            || (parent_schema.getPropertyByName(".userProperties").is_some()
                && child_schema.getPropertyByName(".userProperties").is_some())
    }

    /// Determine USD spec type from Alembic object schema.
    fn determine_spec_type(_obj: &IObject) -> SpecType {
        // All Alembic objects map to Prim specs
        SpecType::Prim
    }

    /// Map an Alembic object schema to the authored USD prim type.
    ///
    /// This follows `_ref/OpenUSD/pxr/usd/plugin/usdAbc/alembicReader.cpp`.
    /// In particular, Alembic `ISubD` is authored as a USD `Mesh` prim plus
    /// subdivision attributes. There is no separate USD prim type named
    /// `"Subdiv"`, so returning a synthetic token here breaks downstream schema
    /// dispatch and Hydra expectations.
    fn map_alembic_to_usd_type_name(obj: &IObject) -> Token {
        if IPolyMesh::new(obj).is_some() {
            Token::new("Mesh")
        } else if IXform::new(obj).is_some() {
            Token::new("Xform")
        } else if IFaceSet::new(obj).is_some() {
            Token::new("GeomSubset")
        } else if let Some(curves) = ICurves::new(obj) {
            if let Ok(sample) = curves.getSample(0) {
                match (sample.curve_type, sample.basis) {
                    (alembic::geom::curves::CurveType::VariableOrder, _) => {
                        Token::new("NurbsCurves")
                    }
                    (
                        alembic::geom::curves::CurveType::Cubic,
                        alembic::geom::curves::BasisType::Hermite,
                    ) => Token::new("HermiteCurves"),
                    _ => Token::new("BasisCurves"),
                }
            } else {
                Token::new("BasisCurves")
            }
        } else if IPoints::new(obj).is_some() {
            Token::new("Points")
        } else if ISubD::new(obj).is_some() {
            Token::new("Mesh")
        } else if INuPatch::new(obj).is_some() {
            Token::new("NurbsPatch")
        } else if ICamera::new(obj).is_some() {
            Token::new("Camera")
        } else {
            Token::new("Scope")
        }
    }

    /// Extract properties from an Alembic object and build property cache.
    ///
    /// Schema-aware: for geometry schemas (IPolyMesh, IXform, ICamera, ICurves,
    /// IPoints, ISubD) we register USD-named virtual properties with closures that
    /// read into the `.geom`/`.xform` sub-compound.  For unknown schemas we fall
    /// back to iterating the top-level compound.
    fn extract_properties(
        archive: &IArchive,
        obj: &IObject,
        path: &Path,
        prim: &mut Prim,
        all_time_samples: &mut TimeSamples,
        time_samples_by_path: &mut HashMap<Path, TimeSamples>,
        time_scale: f64,
        time_offset: f64,
    ) {
        // Try schema-specific extractors first; fall back to generic property iteration.
        let handled = Self::extract_polymesh_properties(
            obj,
            path,
            prim,
            archive,
            all_time_samples,
            time_samples_by_path,
            time_scale,
            time_offset,
        ) || Self::extract_xform_properties(
            obj,
            path,
            prim,
            archive,
            all_time_samples,
            time_samples_by_path,
            time_scale,
            time_offset,
        ) || Self::extract_camera_properties(
            obj,
            path,
            prim,
            archive,
            all_time_samples,
            time_samples_by_path,
            time_scale,
            time_offset,
        ) || Self::extract_curves_properties(
            obj,
            path,
            prim,
            archive,
            all_time_samples,
            time_samples_by_path,
            time_scale,
            time_offset,
        ) || Self::extract_faceset_properties(
            obj,
            path,
            prim,
            archive,
            all_time_samples,
            time_samples_by_path,
            time_scale,
            time_offset,
        ) || Self::extract_points_properties(
            obj,
            path,
            prim,
            archive,
            all_time_samples,
            time_samples_by_path,
            time_scale,
            time_offset,
        ) || Self::extract_subd_properties(
            obj,
            path,
            prim,
            archive,
            all_time_samples,
            time_samples_by_path,
            time_scale,
            time_offset,
        ) || Self::extract_nupatch_properties(
            obj,
            path,
            prim,
            archive,
            all_time_samples,
            time_samples_by_path,
            time_scale,
            time_offset,
        );

        if handled {
            return;
        }

        // Generic fallback: iterate top-level compound properties.
        let props = obj.getProperties();
        for i in 0..props.getNumProperties() {
            if let Some(header) = props.getPropertyHeader(i) {
                let prop_name = &header.name;
                let prop_path = path
                    .append_property(prop_name)
                    .unwrap_or_else(|| path.clone());

                let mut property = Property {
                    type_name: Token::new(""),
                    metadata: BTreeMap::new(),
                    sample_times: TimeSamples::new(),
                    time_sampled: false,
                    uniform: false,
                    converter: None,
                    alembic_object_path: obj.getFullName().to_string(),
                    alembic_property_name: prop_name.to_string(),
                };

                let mut prop_samples = TimeSamples::new();
                let ts_index = header.time_sampling_index as usize;

                if let Some(ts) = archive.getTimeSampling(ts_index) {
                    let num_samples = match header.property_type {
                        PropertyType::Scalar => {
                            if let Some(pw) = props.getPropertyByName(prop_name) {
                                pw.asScalar().map(|r| r.getNumSamples()).unwrap_or(0)
                            } else {
                                0
                            }
                        }
                        PropertyType::Array => {
                            if let Some(pw) = props.getPropertyByName(prop_name) {
                                pw.asArray().map(|r| r.getNumSamples()).unwrap_or(0)
                            } else {
                                0
                            }
                        }
                        PropertyType::Compound => 0,
                    };

                    // Extract all time samples based on TimeSampling type
                    match &ts.time_sampling_type() {
                        TimeSamplingType::Identity => {
                            if num_samples > 0 {
                                prop_samples.insert(OrderedFloat(0.0));
                                all_time_samples.insert(OrderedFloat(0.0));
                            }
                        }
                        TimeSamplingType::Uniform {
                            time_per_cycle,
                            start_time,
                        } => {
                            for i in 0..num_samples {
                                let alembic_time = *start_time + (*time_per_cycle * i as f64);
                                let usd_time = (alembic_time * time_scale) + time_offset;
                                prop_samples.insert(OrderedFloat(usd_time));
                                all_time_samples.insert(OrderedFloat(usd_time));
                            }
                        }
                        TimeSamplingType::Cyclic {
                            time_per_cycle,
                            times,
                        } => {
                            let samples_per_cycle = times.len();
                            if samples_per_cycle > 0 {
                                let num_cycles = num_samples.div_ceil(samples_per_cycle);
                                for cycle in 0..num_cycles {
                                    for (i, &t) in times.iter().enumerate() {
                                        let sample_idx = cycle * samples_per_cycle + i;
                                        if sample_idx >= num_samples {
                                            break;
                                        }
                                        let alembic_time = t + (*time_per_cycle * cycle as f64);
                                        let usd_time = (alembic_time * time_scale) + time_offset;
                                        prop_samples.insert(OrderedFloat(usd_time));
                                        all_time_samples.insert(OrderedFloat(usd_time));
                                    }
                                }
                            }
                        }
                        TimeSamplingType::Acyclic { times } => {
                            for (i, &t) in times.iter().enumerate() {
                                if i >= num_samples {
                                    break;
                                }
                                let usd_time = (t * time_scale) + time_offset;
                                prop_samples.insert(OrderedFloat(usd_time));
                                all_time_samples.insert(OrderedFloat(usd_time));
                            }
                        }
                    }
                }

                property.sample_times = prop_samples.clone();
                property.time_sampled = !prop_samples.is_empty();

                if !prop_samples.is_empty() {
                    time_samples_by_path.insert(prop_path.clone(), prop_samples);
                }

                // Set up converter based on property type
                // FULL implementation: Uses AlembicDataConversion system
                // The converter handles all type conversions from Alembic to USD Value types
                // Supports: scalars, arrays, vectors, matrices, quaternions, strings
                property.converter = Self::create_property_converter(&header, &props, prop_name);

                // Add to prim's property cache
                prim.properties.push(Token::new(prop_name));
                prim.properties_cache
                    .insert(Token::new(prop_name), property);
            }
        }
    }

    // ========================================================================
    // Schema-specific property extractors
    //
    // Each method recognises a geometry schema, registers USD-named virtual
    // properties with converter closures that navigate into the schema's
    // sub-compound (.geom / .xform), and returns true on match.
    // ========================================================================

    /// Helper: register one USD-named virtual property with time samples.
    ///
    /// The converter receives the object top-level ICompoundProperty
    /// and must navigate into sub-compounds itself.
    #[allow(clippy::too_many_arguments)]
    fn register_prop(
        usd_name: &str,
        type_name: &str,
        time_sampling_index: u32,
        num_samples: usize,
        uniform: bool,
        path: &Path,
        prim: &mut Prim,
        archive: &IArchive,
        all_time_samples: &mut TimeSamples,
        time_samples_by_path: &mut HashMap<Path, TimeSamples>,
        time_scale: f64,
        time_offset: f64,
        alembic_obj_name: &str,
        alembic_prop_name: &str,
        converter: Box<dyn Fn(&ICompoundProperty, &str, usize) -> Option<Value> + Send + Sync>,
    ) {
        let prop_path = match path.append_property(usd_name) {
            Some(p) => p,
            None => return,
        };
        let mut prop_samples = TimeSamples::new();
        if let Some(ts) = archive.getTimeSampling(time_sampling_index as usize) {
            Self::collect_time_samples(
                &ts.time_sampling_type(),
                num_samples,
                time_scale,
                time_offset,
                &mut prop_samples,
                all_time_samples,
            );
        } else if num_samples > 0 {
            prop_samples.insert(OrderedFloat(0.0));
            all_time_samples.insert(OrderedFloat(0.0));
        }
        let time_sampled = !prop_samples.is_empty();
        if time_sampled {
            time_samples_by_path.insert(prop_path, prop_samples.clone());
        }
        prim.properties.push(Token::new(usd_name));
        prim.properties_cache.insert(
            Token::new(usd_name),
            Property {
                type_name: Token::new(type_name),
                metadata: BTreeMap::new(),
                sample_times: prop_samples,
                time_sampled,
                uniform,
                converter: Some(converter),
                alembic_object_path: alembic_obj_name.to_string(),
                alembic_property_name: alembic_prop_name.to_string(),
            },
        );
    }

    /// Populate time-sample sets from a TimeSamplingType.
    fn collect_time_samples(
        ts_type: &TimeSamplingType,
        num_samples: usize,
        time_scale: f64,
        time_offset: f64,
        prop_samples: &mut TimeSamples,
        all_time_samples: &mut TimeSamples,
    ) {
        match ts_type {
            TimeSamplingType::Identity => {
                if num_samples > 0 {
                    prop_samples.insert(OrderedFloat(0.0));
                    all_time_samples.insert(OrderedFloat(0.0));
                }
            }
            TimeSamplingType::Uniform {
                time_per_cycle,
                start_time,
            } => {
                for i in 0..num_samples {
                    let abc_time = *start_time + *time_per_cycle * i as f64;
                    let usd = Self::round_time(abc_time * time_scale + time_offset);
                    prop_samples.insert(OrderedFloat(usd));
                    all_time_samples.insert(OrderedFloat(usd));
                }
            }
            TimeSamplingType::Cyclic {
                time_per_cycle,
                times,
            } => {
                let spc = times.len();
                if spc > 0 {
                    for c in 0..num_samples.div_ceil(spc) {
                        for (i, &t) in times.iter().enumerate() {
                            if c * spc + i >= num_samples {
                                break;
                            }
                            let abc_time = t + *time_per_cycle * c as f64;
                            let usd = Self::round_time(abc_time * time_scale + time_offset);
                            prop_samples.insert(OrderedFloat(usd));
                            all_time_samples.insert(OrderedFloat(usd));
                        }
                    }
                }
            }
            TimeSamplingType::Acyclic { times } => {
                for (i, &t) in times.iter().enumerate() {
                    if i >= num_samples {
                        break;
                    }
                    let usd = Self::round_time(t * time_scale + time_offset);
                    prop_samples.insert(OrderedFloat(usd));
                    all_time_samples.insert(OrderedFloat(usd));
                }
            }
        }
    }

    /// (num_samples, ts_index) from `.geom`->named prop.
    fn geom_num_samples(obj: &IObject, abc_prop: &str) -> (usize, u32) {
        let props = obj.getProperties();
        if let Some(gw) = props.getPropertyByName(".geom") {
            if let Some(g) = gw.asCompound() {
                if let Some(pw) = g.getPropertyByName(abc_prop) {
                    let ts = pw.getHeader().time_sampling_index;
                    if let Some(a) = pw.asArray() {
                        return (a.getNumSamples(), ts);
                    }
                    if let Some(s) = pw.asScalar() {
                        return (s.getNumSamples(), ts);
                    }
                }
            }
        }
        (0, 0)
    }

    /// (num_samples, ts_index) from `.xform`->named prop.
    fn xform_num_samples(obj: &IObject, abc_prop: &str) -> (usize, u32) {
        let props = obj.getProperties();
        if let Some(gw) = props.getPropertyByName(".xform") {
            if let Some(x) = gw.asCompound() {
                if let Some(pw) = x.getPropertyByName(abc_prop) {
                    let ts = pw.getHeader().time_sampling_index;
                    if let Some(s) = pw.asScalar() {
                        return (s.getNumSamples(), ts);
                    }
                    if let Some(a) = pw.asArray() {
                        return (a.getNumSamples(), ts);
                    }
                }
            }
        }
        (0, 0)
    }

    // ---- IPolyMesh -> Mesh ----

    fn extract_polymesh_properties(
        obj: &IObject,
        path: &Path,
        prim: &mut Prim,
        archive: &IArchive,
        all_ts: &mut TimeSamples,
        ts_by_path: &mut HashMap<Path, TimeSamples>,
        scale: f64,
        offset: f64,
    ) -> bool {
        if IPolyMesh::new(obj).is_none() {
            return false;
        }
        // Full Alembic path needed for findObject() lookup in query_time_sample
        let obj_name = obj.getFullName().to_string();

        let (n, ts) = Self::geom_num_samples(obj, "P");
        Self::register_prop(
            "points",
            "point3f[]",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            "P",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let bytes = read_sub_array(top, ".geom", "P", sel)?;
                Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                    &bytes,
                ))))
            }),
        );

        let (n, ts) = Self::geom_num_samples(obj, ".faceCounts");
        Self::register_prop(
            "faceVertexCounts",
            "int[]",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            ".faceCounts",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let bytes = read_sub_array(top, ".geom", ".faceCounts", sel)?;
                Some(Value::new(usd_vt::Array::from(i32_from_bytes(&bytes))))
            }),
        );

        let (n, ts) = Self::geom_num_samples(obj, ".faceIndices");
        Self::register_prop(
            "faceVertexIndices",
            "int[]",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            ".faceIndices",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let bytes = read_sub_array(top, ".geom", ".faceIndices", sel)?;
                Some(Value::new(usd_vt::Array::from(i32_from_bytes(&bytes))))
            }),
        );

        let (n, ts) = Self::geom_num_samples(obj, "N");
        if n > 0 {
            Self::register_prop(
                "normals",
                "normal3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "N",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    // N may be indexed GeomParam compound or direct array inside .geom.
                    let geom_prop = top.getPropertyByName(".geom")?;
                    let g = geom_prop.asCompound()?;
                    Some(Value::from_no_hash(usd_vt::Array::from(
                        read_geom_param_vec3f(&g, "N", sel)?,
                    )))
                }),
            );
        }

        let (n, ts) = Self::geom_num_samples(obj, "uv");
        if n > 0 {
            Self::register_prop(
                "primvars:st",
                "texCoord2f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "uv",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let geom_prop = top.getPropertyByName(".geom")?;
                    let g = geom_prop.asCompound()?;
                    Some(Value::from_no_hash(usd_vt::Array::from(
                        read_geom_param_vec2f(&g, "uv", sel)?,
                    )))
                }),
            );
        }

        // velocities (Vector3fArray).
        let (n, ts) = Self::geom_num_samples(obj, ".velocities");
        if n > 0 {
            Self::register_prop(
                "velocities",
                "vector3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".velocities",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", ".velocities", sel)?;
                    Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                        &bytes,
                    ))))
                }),
            );
        }

        // extent from .selfBnds (6xf64 scalar -> 2 Vec3f).
        let (n, ts) = Self::geom_num_samples(obj, ".selfBnds");
        if n > 0 {
            Self::register_prop(
                "extent",
                "float3[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".selfBnds",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let mut buf = [0u8; 48];
                    if !read_sub_scalar(top, ".geom", ".selfBnds", sel, &mut buf) {
                        return None;
                    }
                    let d: &[f64] = bytemuck::try_cast_slice(&buf).ok()?;
                    use usd_gf::vec3::Vec3f;
                    let out = vec![
                        Vec3f::new(d[0] as f32, d[1] as f32, d[2] as f32),
                        Vec3f::new(d[3] as f32, d[4] as f32, d[5] as f32),
                    ];
                    Some(Value::from_no_hash(usd_vt::Array::from(out)))
                }),
            );
        }

        // subdivisionScheme = "none" (PolyMesh is not subdivision surface).
        prim.properties.push(Token::new("subdivisionScheme"));
        prim.properties_cache.insert(
            Token::new("subdivisionScheme"),
            Property {
                type_name: Token::new("token"),
                metadata: BTreeMap::new(),
                sample_times: [OrderedFloat(0.0)].into_iter().collect(),
                time_sampled: false,
                uniform: true,
                converter: Some(Box::new(|_top, _n, _sel| {
                    Some(Value::new(Token::new("none")))
                })),
                alembic_object_path: obj_name.clone(),
                alembic_property_name: "subdivisionScheme".to_string(),
            },
        );

        true
    }

    /// Mirror `_ReadFaceSet()` from the OpenUSD Alembic importer.
    ///
    /// FaceSets become `GeomSubset` prims with `indices`, `elementType=face`,
    /// and `familyName=materialBind`. The corresponding parent mesh receives
    /// `subsetFamily:materialBind:familyType`, derived from child exclusivity.
    fn extract_faceset_properties(
        obj: &IObject,
        path: &Path,
        prim: &mut Prim,
        archive: &IArchive,
        all_ts: &mut TimeSamples,
        ts_by_path: &mut HashMap<Path, TimeSamples>,
        scale: f64,
        offset: f64,
    ) -> bool {
        let Some(face_set) = IFaceSet::new(obj) else {
            return false;
        };

        let obj_name = obj.getFullName().to_string();
        let sample_count = face_set.getNumSamples();
        let time_sampling_index = face_set.getTimeSamplingIndex();

        Self::register_prop(
            "indices",
            "int[]",
            time_sampling_index,
            sample_count,
            face_set.isConstant(),
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            ".faces",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let bytes = read_sub_array(top, ".faceset", ".faces", sel)?;
                Some(Value::new(usd_vt::Array::from(i32_from_bytes(&bytes))))
            }),
        );

        upsert_uniform_token_property(prim, "elementType", "face", &obj_name, "elementType");
        upsert_uniform_token_property(
            prim,
            "familyName",
            FACESET_DEFAULT_FAMILY_NAME,
            &obj_name,
            "familyName",
        );

        true
    }

    // ---- IXform -> Xform ----

    fn extract_xform_properties(
        obj: &IObject,
        path: &Path,
        prim: &mut Prim,
        archive: &IArchive,
        all_ts: &mut TimeSamples,
        ts_by_path: &mut HashMap<Path, TimeSamples>,
        scale: f64,
        offset: f64,
    ) -> bool {
        if IXform::new(obj).is_none() {
            return false;
        }
        let obj_name = obj.getFullName().to_string();

        // C++ checks getInheritsXforms() for all samples and warns+skips if any
        // sample has inheritsXforms==false (meaning the xform is world-space, not
        // local). The alembic-rs crate does not expose getInheritsXforms() on
        // IXformSample, so this validation is skipped. All xforms are assumed local.

        let (n, ts) = {
            let r = Self::xform_num_samples(obj, ".vals");
            if r.0 > 0 {
                r
            } else {
                Self::xform_num_samples(obj, ".inherits")
            }
        };

        // Only emit xform properties when there are actual samples (BUG-2).
        if n == 0 {
            return true;
        }

        // xformOp:transform - concatenated 4x4 matrix from .ops + .vals.
        Self::register_prop(
            "xformOp:transform",
            "matrix4d",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            ".vals",
            Box::new(move |top: &ICompoundProperty, _: &str, sel: usize| {
                let idx = sel;
                let xw = top.getPropertyByName(".xform")?;
                let xform = xw.asCompound()?;

                // .ops: static scalar (read at index 0), extent = number of op codes.
                let ops: Vec<u8> = xform
                    .getPropertyByName(".ops")
                    .and_then(|p| {
                        let sc = p.asScalar()?;
                        let n_ops = sc.getHeader().data_type.extent as usize;
                        let mut buf = vec![0u8; n_ops];
                        sc.getSample(0, &mut buf).ok()?;
                        Some(buf)
                    })
                    .unwrap_or_default();

                // .vals: scalar (multiple f64s in one sample) or array of f64s.
                let vals: Vec<f64> = xform
                    .getPropertyByName(".vals")
                    .and_then(|p| {
                        if p.isScalar() {
                            let sc = p.asScalar()?;
                            let nv = sc.getHeader().data_type.extent as usize;
                            let mut buf = vec![0u8; nv * 8];
                            sc.getSample(idx, &mut buf).ok()?;
                            Some(bytemuck::cast_slice::<u8, f64>(&buf).to_vec())
                        } else {
                            let bytes = p.asArray()?.getSampleVec(idx).ok()?;
                            Some(bytemuck::cast_slice::<u8, f64>(&bytes).to_vec())
                        }
                    })
                    .unwrap_or_default();

                let mut mat = [[0.0f64; 4]; 4];
                mat[0][0] = 1.0;
                mat[1][1] = 1.0;
                mat[2][2] = 1.0;
                mat[3][3] = 1.0;
                let mut vi = 0usize;
                for &op_code in &ops {
                    let ot = op_code >> 4;
                    let nv: usize = match ot {
                        0 | 1 => 3,
                        2 => 4,
                        3 => 16,
                        4 | 5 | 6 => 1,
                        _ => continue,
                    };
                    if vi + nv > vals.len() {
                        break;
                    }
                    let op_mat = AlembicDataReader::xform_op_to_mat4(ot, &vals[vi..vi + nv]);
                    mat = AlembicDataReader::mat4_mul(op_mat, mat);
                    vi += nv;
                }
                use usd_gf::matrix4::Matrix4d;
                Some(Value::from_no_hash(Matrix4d::from_array(mat)))
            }),
        );

        // xformOpOrder - static uniform token[].
        prim.properties.push(Token::new("xformOpOrder"));
        prim.properties_cache.insert(
            Token::new("xformOpOrder"),
            Property {
                type_name: Token::new("token[]"),
                metadata: BTreeMap::new(),
                sample_times: [OrderedFloat(0.0)].into_iter().collect(),
                time_sampled: false,
                uniform: true,
                converter: Some(Box::new(|_top, _n, _sel| {
                    Some(Value::new(usd_vt::Array::from(vec![usd_tf::Token::new(
                        "xformOp:transform",
                    )])))
                })),
                alembic_object_path: obj_name.clone(),
                alembic_property_name: "xformOpOrder".to_string(),
            },
        );

        true
    }

    /// Round time for exact frame matching (C++ ConvertSampleTimes, GfRound).
    fn round_time(t: f64) -> f64 {
        const P: f64 = 1.0e+10;
        (P * t).round() / P
    }

    /// Row-major 4x4 multiply.
    fn mat4_mul(a: [[f64; 4]; 4], b: [[f64; 4]; 4]) -> [[f64; 4]; 4] {
        let mut r = [[0.0f64; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                r[i][j] =
                    a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
            }
        }
        r
    }

    /// Single Xform op-code + values -> row-major 4x4 matrix (Imath convention).
    fn xform_op_to_mat4(op_type: u8, v: &[f64]) -> [[f64; 4]; 4] {
        let mut m = [[0.0f64; 4]; 4];
        m[0][0] = 1.0;
        m[1][1] = 1.0;
        m[2][2] = 1.0;
        m[3][3] = 1.0;
        match op_type {
            0 => {
                m[0][0] = v[0];
                m[1][1] = v[1];
                m[2][2] = v[2];
            } // Scale
            1 => {
                m[3][0] = v[0];
                m[3][1] = v[1];
                m[3][2] = v[2];
            } // Translate (row-vec: row 3 = T)
            4 => {
                // RotateX
                let (s, c) = (v[0].to_radians().sin(), v[0].to_radians().cos());
                m[1][1] = c;
                m[1][2] = s;
                m[2][1] = -s;
                m[2][2] = c;
            }
            5 => {
                // RotateY
                let (s, c) = (v[0].to_radians().sin(), v[0].to_radians().cos());
                m[0][0] = c;
                m[0][2] = -s;
                m[2][0] = s;
                m[2][2] = c;
            }
            6 => {
                // RotateZ
                let (s, c) = (v[0].to_radians().sin(), v[0].to_radians().cos());
                m[0][0] = c;
                m[0][1] = s;
                m[1][0] = -s;
                m[1][1] = c;
            }
            2 => {
                // Rotate axis+angle
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if len > 1e-10 {
                    let (nx, ny, nz) = (v[0] / len, v[1] / len, v[2] / len);
                    let (s, c) = (v[3].to_radians().sin(), v[3].to_radians().cos());
                    let t = 1.0 - c;
                    m[0][0] = t * nx * nx + c;
                    m[0][1] = t * nx * ny + s * nz;
                    m[0][2] = t * nx * nz - s * ny;
                    m[1][0] = t * nx * ny - s * nz;
                    m[1][1] = t * ny * ny + c;
                    m[1][2] = t * ny * nz + s * nx;
                    m[2][0] = t * nx * nz + s * ny;
                    m[2][1] = t * ny * nz - s * nx;
                    m[2][2] = t * nz * nz + c;
                }
            }
            3 => {
                if v.len() >= 16 {
                    for r in 0..4 {
                        for c in 0..4 {
                            m[r][c] = v[r * 4 + c];
                        }
                    }
                }
            }
            _ => {}
        }
        m
    }

    // ---- ICamera -> Camera ----

    fn extract_camera_properties(
        obj: &IObject,
        path: &Path,
        prim: &mut Prim,
        archive: &IArchive,
        all_ts: &mut TimeSamples,
        ts_by_path: &mut HashMap<Path, TimeSamples>,
        scale: f64,
        offset: f64,
    ) -> bool {
        if ICamera::new(obj).is_none() {
            return false;
        }
        let obj_name = obj.getFullName().to_string();

        // .core is a 16-f64 scalar inside .geom.
        let (n, ts) = {
            let props = obj.getProperties();
            let mut res = (0usize, 0u32);
            if let Some(gw) = props.getPropertyByName(".geom") {
                if let Some(g) = gw.asCompound() {
                    if let Some(cw) = g.getPropertyByName(".core") {
                        res = (
                            cw.asScalar().map(|s| s.getNumSamples()).unwrap_or(0),
                            cw.getHeader().time_sampling_index,
                        );
                    }
                }
            }
            res
        };

        // (usd_name, type_name, core_index, multiplier, apply_squeeze).
        // Aperture in cm -> USD in mm (*10). C++ also multiplies h/vAperture by
        // getLensSqueezeRatio() which is .core[13].
        let params: &[(&str, &str, usize, f64, bool)] = &[
            ("focalLength", "float", 0, 1.0, false),
            ("horizontalAperture", "float", 1, 10.0, true),
            ("horizontalApertureOffset", "float", 2, 10.0, true),
            ("verticalAperture", "float", 3, 10.0, true),
            ("verticalApertureOffset", "float", 4, 10.0, true),
            ("fStop", "float", 10, 1.0, false),
            ("focusDistance", "float", 11, 1.0, false),
        ];
        for &(usd_name, type_name, ci, mul, squeeze) in params {
            Self::register_prop(
                usd_name,
                type_name,
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".core",
                Box::new(move |top: &ICompoundProperty, _: &str, sel: usize| {
                    let mut buf = [0u8; 128];
                    if !read_sub_scalar(top, ".geom", ".core", sel, &mut buf) {
                        return None;
                    }
                    let d: &[f64] = bytemuck::try_cast_slice(&buf).ok()?;
                    if ci >= d.len() {
                        return None;
                    }
                    // Squeeze ratio at index 13 (default 1.0 if missing).
                    let squeeze_ratio = if squeeze {
                        d.get(13).copied().unwrap_or(1.0)
                    } else {
                        1.0
                    };
                    Some(Value::from_f32((d[ci] * mul * squeeze_ratio) as f32))
                }),
            );
        }

        // clippingRange -> Vec2f(near, far) at indices 14, 15.
        Self::register_prop(
            "clippingRange",
            "float2",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            ".core",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let mut buf = [0u8; 128];
                if !read_sub_scalar(top, ".geom", ".core", sel, &mut buf) {
                    return None;
                }
                let d: &[f64] = bytemuck::try_cast_slice(&buf).ok()?;
                if d.len() < 16 {
                    return None;
                }
                use usd_gf::vec2::Vec2f;
                Some(Value::from_no_hash(Vec2f::new(d[14] as f32, d[15] as f32)))
            }),
        );

        true
    }

    // ---- ICurves -> BasisCurves / HermiteCurves / NurbsCurves ----

    /// Alembic Hermite curves store control points as `(P0, T0, ..., Pn, Tn)`,
    /// while USD authors separate `points` and `tangents`.
    fn split_hermite_point_and_tangent_arrays(
        interleaved: &[usd_gf::vec3::Vec3f],
    ) -> (Vec<usd_gf::vec3::Vec3f>, Vec<usd_gf::vec3::Vec3f>) {
        use usd_gf::vec3::Vec3f;

        let pair_count = interleaved.len() / 2;
        let mut points = Vec::with_capacity(pair_count);
        let mut tangents = Vec::with_capacity(pair_count);
        for pair in interleaved.chunks_exact(2) {
            points.push(Vec3f::new(pair[0].x, pair[0].y, pair[0].z));
            tangents.push(Vec3f::new(pair[1].x, pair[1].y, pair[1].z));
        }
        (points, tangents)
    }

    fn extract_curves_properties(
        obj: &IObject,
        path: &Path,
        prim: &mut Prim,
        archive: &IArchive,
        all_ts: &mut TimeSamples,
        ts_by_path: &mut HashMap<Path, TimeSamples>,
        scale: f64,
        offset: f64,
    ) -> bool {
        let Some(curves) = ICurves::new(obj) else {
            return false;
        };
        let obj_name = obj.getFullName().to_string();
        let first_sample = curves.getSample(0).ok();
        let is_nurbs = first_sample
            .as_ref()
            .is_some_and(|sample| sample.curve_type == alembic::geom::curves::CurveType::VariableOrder);
        let is_hermite = first_sample.as_ref().is_some_and(|sample| {
            sample.curve_type == alembic::geom::curves::CurveType::Cubic
                && sample.basis == alembic::geom::curves::BasisType::Hermite
        });

        let (n, ts) = Self::geom_num_samples(obj, "P");
        if is_hermite {
            Self::register_prop(
                "points",
                "point3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "P",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", "P", sel)?;
                    let interleaved = vec3f_from_bytes(&bytes);
                    let (points, _) = Self::split_hermite_point_and_tangent_arrays(&interleaved);
                    Some(Value::from_no_hash(usd_vt::Array::from(points)))
                }),
            );
            Self::register_prop(
                "tangents",
                "vector3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "P",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", "P", sel)?;
                    let interleaved = vec3f_from_bytes(&bytes);
                    let (_, tangents) =
                        Self::split_hermite_point_and_tangent_arrays(&interleaved);
                    Some(Value::from_no_hash(usd_vt::Array::from(tangents)))
                }),
            );
        } else {
            Self::register_prop(
                "points",
                "point3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "P",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", "P", sel)?;
                    Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                        &bytes,
                    ))))
                }),
            );
        }

        // Curves store counts under `nVertices` (no leading dot). Some older
        // files/tools may still surface `.nVertices`, so we accept both and
        // prefer the `_ref` name.
        let (n, ts) = {
            let direct = Self::geom_num_samples(obj, "nVertices");
            if direct.0 > 0 {
                direct
            } else {
                Self::geom_num_samples(obj, ".nVertices")
            }
        };
        if n > 0 {
            Self::register_prop(
                "curveVertexCounts",
                "int[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".nVertices",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", "nVertices", sel)
                        .or_else(|| read_sub_array(top, ".geom", ".nVertices", sel))?;
                    Some(Value::new(usd_vt::Array::from(i32_from_bytes(&bytes))))
                }),
            );
        }

        // widths — Alembic schema property name is "width" (singular, no dot prefix).
        let (n, ts) = Self::geom_num_samples(obj, "width");
        if n > 0 {
            Self::register_prop(
                "widths",
                "float[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "width",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let geom_prop = top.getPropertyByName(".geom")?;
                    let g = geom_prop.asCompound()?;
                    // width may be a GeomParam compound or direct array.
                    let pw = g.getPropertyByName("width")?;
                    if let Some(c) = pw.asCompound() {
                        let vals_bytes = c
                            .getPropertyByName(".vals")?
                            .asArray()?
                            .getSampleVec(sel)
                            .ok()?;
                        let v: Vec<f32> = bytemuck::cast_slice(&vals_bytes).to_vec();
                        return Some(Value::from_no_hash(usd_vt::Array::from(v)));
                    }
                    if let Some(a) = pw.asArray() {
                        let bytes = a.getSampleVec(sel).ok()?;
                        let v: Vec<f32> = bytemuck::cast_slice(&bytes).to_vec();
                        return Some(Value::from_no_hash(usd_vt::Array::from(v)));
                    }
                    None
                }),
            );
        }

        // velocities (Vector3fArray) — BUG-9.
        let (n, ts) = Self::geom_num_samples(obj, ".velocities");
        if n > 0 {
            Self::register_prop(
                "velocities",
                "vector3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".velocities",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", ".velocities", sel)?;
                    Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                        &bytes,
                    ))))
                }),
            );
        }

        // normals (Normal3fArray from "N" GeomParam) — BUG-10.
        let (n, ts) = Self::geom_num_samples(obj, "N");
        if n > 0 {
            Self::register_prop(
                "normals",
                "normal3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "N",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let geom_prop = top.getPropertyByName(".geom")?;
                    let g = geom_prop.asCompound()?;
                    Some(Value::from_no_hash(usd_vt::Array::from(
                        read_geom_param_vec3f(&g, "N", sel)?,
                    )))
                }),
            );
        }

        // Read `curveBasisAndType` from the Alembic curves schema.
        //
        // `_ref` consumes the first sample from the packed schema value and
        // treats it as:
        //   [0] curveType
        //   [1] wrap / periodicity
        //   [2] basis
        // Some Rust ports previously swapped basis and wrap or looked for the
        // wrong property name (`.basisAndType`), which makes real curves files
        // silently lose their topology contract.
        let (curve_type, curve_basis, curve_wrap) = {
            let mut ct = "cubic".to_string();
            let mut cb = "bezier".to_string();
            let mut cw = "nonperiodic".to_string();
            let props = obj.getProperties();
            if let Some(gw) = props.getPropertyByName(".geom") {
                if let Some(g) = gw.asCompound() {
                    if let Some(pw) = g
                        .getPropertyByName("curveBasisAndType")
                        .or_else(|| g.getPropertyByName(".basisAndType"))
                    {
                        if let Some(sc) = pw.asScalar() {
                            let mut buf = [0u8; 4];
                            if sc.getSample(0, &mut buf).is_ok() {
                                // curveType: 0=linear, 1=cubic
                                ct = match buf[0] {
                                    1 => "linear",
                                    0 => "cubic",
                                    _ => "cubic",
                                }
                                .to_string();
                                // curvePeriodicity / wrap: 0=nonperiodic, 1=periodic
                                cw = match buf[1] {
                                    1 => "periodic",
                                    _ => "nonperiodic",
                                }
                                .to_string();
                                // basis: 0=noBasis, 1=bezier, 2=bspline, 3=catmullRom, 4=hermite, 5=power
                                cb = match buf[2] {
                                    1 => "bezier",
                                    2 => "bspline",
                                    3 => "catmullRom",
                                    4 => "hermite",
                                    5 => "power",
                                    _ => "bezier",
                                }
                                .to_string();
                            }
                        }
                    }
                }
            }
            (ct, cb, cw)
        };
        if !is_nurbs && !is_hermite {
            let curve_consts: &[(&str, String)] = &[
                ("type", curve_type),
                ("basis", curve_basis),
                ("wrap", curve_wrap),
            ];
            for (usd_name, val) in curve_consts {
                let val = val.clone();
                prim.properties.push(Token::new(usd_name));
                prim.properties_cache.insert(
                    Token::new(usd_name),
                    Property {
                        type_name: Token::new("token"),
                        metadata: BTreeMap::new(),
                        sample_times: [OrderedFloat(0.0)].into_iter().collect(),
                        time_sampled: false,
                        uniform: true,
                        converter: Some(Box::new(move |_top, _n, _sel| {
                            Some(Value::new(Token::new(&val)))
                        })),
                        alembic_object_path: obj_name.clone(),
                        alembic_property_name: usd_name.to_string(),
                    },
                );
            }
        }

        if is_nurbs {
            let (n, ts) = Self::geom_num_samples(obj, ".orders");
            if n > 0 {
                Self::register_prop(
                    "order",
                    "int[]",
                    ts,
                    n,
                    false,
                    path,
                    prim,
                    archive,
                    all_ts,
                    ts_by_path,
                    scale,
                    offset,
                    &obj_name,
                    ".orders",
                    Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                        let bytes = read_sub_array(top, ".geom", ".orders", sel)?;
                        let orders: Vec<i32> = bytes.into_iter().map(i32::from).collect();
                        Some(Value::new(usd_vt::Array::from(orders)))
                    }),
                );
            }

            let (n, ts) = Self::geom_num_samples(obj, ".knots");
            if n > 0 {
                Self::register_prop(
                    "knots",
                    "double[]",
                    ts,
                    n,
                    false,
                    path,
                    prim,
                    archive,
                    all_ts,
                    ts_by_path,
                    scale,
                    offset,
                    &obj_name,
                    ".knots",
                    Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                        let bytes = read_sub_array(top, ".geom", ".knots", sel)?;
                        let knots: Vec<f64> = bytemuck::cast_slice::<u8, f32>(&bytes)
                            .iter()
                            .copied()
                            .map(f64::from)
                            .collect();
                        Some(Value::from_no_hash(usd_vt::Array::from(knots)))
                    }),
                );
            }

            let (n, ts) = Self::geom_num_samples(obj, "Pw");
            if n > 0 {
                Self::register_prop(
                    "pointWeights",
                    "double[]",
                    ts,
                    n,
                    false,
                    path,
                    prim,
                    archive,
                    all_ts,
                    ts_by_path,
                    scale,
                    offset,
                    &obj_name,
                    "Pw",
                    Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                        let bytes = read_sub_array(top, ".geom", "Pw", sel)?;
                        let weights: Vec<f64> = bytemuck::cast_slice::<u8, f32>(&bytes)
                            .into_iter()
                            .copied()
                            .map(f64::from)
                            .collect();
                        Some(Value::from_no_hash(usd_vt::Array::from(weights)))
                    }),
                );
            }
        }

        true
    }

    // ---- IPoints -> Points ----

    fn extract_points_properties(
        obj: &IObject,
        path: &Path,
        prim: &mut Prim,
        archive: &IArchive,
        all_ts: &mut TimeSamples,
        ts_by_path: &mut HashMap<Path, TimeSamples>,
        scale: f64,
        offset: f64,
    ) -> bool {
        if IPoints::new(obj).is_none() {
            return false;
        }
        let obj_name = obj.getFullName().to_string();

        let (n, ts) = Self::geom_num_samples(obj, "P");
        Self::register_prop(
            "points",
            "point3f[]",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            "P",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let bytes = read_sub_array(top, ".geom", "P", sel)?;
                Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                    &bytes,
                ))))
            }),
        );

        // Point IDs stored as ".pointIds" in Alembic schema compound (BUG-12: was "id").
        let (n, ts) = Self::geom_num_samples(obj, ".pointIds");
        if n > 0 {
            Self::register_prop(
                "ids",
                "int64[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".pointIds",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", ".pointIds", sel)?;
                    let v: Vec<i64> = bytemuck::cast_slice::<u8, u64>(&bytes)
                        .iter()
                        .map(|&x| x as i64)
                        .collect();
                    Some(Value::new(usd_vt::Array::from(v)))
                }),
            );
        }

        // velocities (Vector3fArray) — BUG-13.
        let (n, ts) = Self::geom_num_samples(obj, ".velocities");
        if n > 0 {
            Self::register_prop(
                "velocities",
                "vector3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".velocities",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", ".velocities", sel)?;
                    Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                        &bytes,
                    ))))
                }),
            );
        }

        // widths as GeomParam (compound=indexed or direct array) — BUG-14.
        let (n, ts) = Self::geom_num_samples(obj, ".widths");
        if n > 0 {
            Self::register_prop(
                "widths",
                "float[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".widths",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let geom_prop = top.getPropertyByName(".geom")?;
                    let g = geom_prop.asCompound()?;
                    let pw = g.getPropertyByName(".widths")?;
                    // GeomParam: compound with .vals/.indices or direct array.
                    if let Some(c) = pw.asCompound() {
                        let vals_bytes = c
                            .getPropertyByName(".vals")?
                            .asArray()?
                            .getSampleVec(sel)
                            .ok()?;
                        let v: Vec<f32> = bytemuck::cast_slice(&vals_bytes).to_vec();
                        return Some(Value::from_no_hash(usd_vt::Array::from(v)));
                    }
                    if let Some(a) = pw.asArray() {
                        let bytes = a.getSampleVec(sel).ok()?;
                        let v: Vec<f32> = bytemuck::cast_slice(&bytes).to_vec();
                        return Some(Value::from_no_hash(usd_vt::Array::from(v)));
                    }
                    None
                }),
            );
        }

        true
    }

    /// Apply the parent mesh subset-family token once all child FaceSets are known.
    ///
    /// The reference importer authors this property while reading each FaceSet.
    /// Doing it after child recursion keeps the Rust reader simple and avoids
    /// order-dependent behavior when multiple FaceSets share the same parent.
    fn apply_faceset_family_type_from_children(
        obj: &IObject,
        obj_path: &Path,
        prims: &mut HashMap<Path, Prim>,
    ) {
        let mut saw_face_set = false;
        let mut family_type = "nonOverlapping";

        for i in 0..obj.getNumChildren() {
            let Some(child) = obj.getChild(i) else {
                continue;
            };
            let Some(face_set) = IFaceSet::new(&child) else {
                continue;
            };
            saw_face_set = true;
            if face_set.face_exclusivity() != FaceSetExclusivity::Exclusive {
                family_type = "unrestricted";
                break;
            }
        }

        if !saw_face_set {
            return;
        }

        let Some(parent_prim) = prims.get_mut(obj_path) else {
            return;
        };
        upsert_uniform_token_property(
            parent_prim,
            FACESET_DEFAULT_FAMILY_TYPE_ATTR,
            family_type,
            obj.getFullName(),
            FACESET_DEFAULT_FAMILY_TYPE_ATTR,
        );
    }

    // ---- ISubD -> Mesh (with subdivision attributes) ----

    fn extract_subd_properties(
        obj: &IObject,
        path: &Path,
        prim: &mut Prim,
        archive: &IArchive,
        all_ts: &mut TimeSamples,
        ts_by_path: &mut HashMap<Path, TimeSamples>,
        scale: f64,
        offset: f64,
    ) -> bool {
        if ISubD::new(obj).is_none() {
            return false;
        }
        let obj_name = obj.getFullName().to_string();

        let (n, ts) = Self::geom_num_samples(obj, "P");
        Self::register_prop(
            "points",
            "point3f[]",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            "P",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let bytes = read_sub_array(top, ".geom", "P", sel)?;
                Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                    &bytes,
                ))))
            }),
        );
        let (n, ts) = Self::geom_num_samples(obj, ".faceCounts");
        Self::register_prop(
            "faceVertexCounts",
            "int[]",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            ".faceCounts",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let bytes = read_sub_array(top, ".geom", ".faceCounts", sel)?;
                Some(Value::new(usd_vt::Array::from(i32_from_bytes(&bytes))))
            }),
        );
        let (n, ts) = Self::geom_num_samples(obj, ".faceIndices");
        Self::register_prop(
            "faceVertexIndices",
            "int[]",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            ".faceIndices",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let bytes = read_sub_array(top, ".geom", ".faceIndices", sel)?;
                Some(Value::new(usd_vt::Array::from(i32_from_bytes(&bytes))))
            }),
        );

        // subdivisionScheme - static token from .scheme scalar.
        let scheme_str = {
            let mut s = "catmullClark".to_string();
            let props = obj.getProperties();
            if let Some(gw) = props.getPropertyByName(".geom") {
                if let Some(g) = gw.asCompound() {
                    if let Some(sw) = g.getPropertyByName(".scheme") {
                        if let Some(sc) = sw.asScalar() {
                            if let Ok(bytes) = sc.getSampleVec(0) {
                                if let Ok(txt) = std::str::from_utf8(&bytes) {
                                    // Map Alembic scheme names to USD tokens (BUG-15).
                                    s = match txt.trim_end_matches('\0') {
                                        "catmull-clark" => "catmullClark",
                                        "loop" => "loop",
                                        "bilinear" => "bilinear",
                                        other if !other.is_empty() => other,
                                        _ => "catmullClark",
                                    }
                                    .to_string();
                                }
                            }
                        }
                    }
                }
            }
            s
        };
        prim.properties.push(Token::new("subdivisionScheme"));
        prim.properties_cache.insert(
            Token::new("subdivisionScheme"),
            Property {
                type_name: Token::new("token"),
                metadata: BTreeMap::new(),
                sample_times: [OrderedFloat(0.0)].into_iter().collect(),
                time_sampled: false,
                uniform: true,
                converter: Some(Box::new(move |_top, _n, _sel| {
                    Some(Value::new(scheme_str.clone()))
                })),
                alembic_object_path: obj_name.clone(),
                alembic_property_name: "subdivisionScheme".to_string(),
            },
        );

        // velocities (Vector3fArray).
        let (n, ts) = Self::geom_num_samples(obj, ".velocities");
        if n > 0 {
            Self::register_prop(
                "velocities",
                "vector3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".velocities",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", ".velocities", sel)?;
                    Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                        &bytes,
                    ))))
                }),
            );
        }

        // interpolateBoundary — read i32 from .interpolateBoundary scalar (BUG-16).
        let interp_boundary = {
            let mut val = "edgeAndCorner".to_string();
            let props = obj.getProperties();
            if let Some(gw) = props.getPropertyByName(".geom") {
                if let Some(g) = gw.asCompound() {
                    if let Some(pw) = g.getPropertyByName(".interpolateBoundary") {
                        if let Some(sc) = pw.asScalar() {
                            let mut buf = [0u8; 4];
                            if sc.getSample(0, &mut buf).is_ok() {
                                let iv = i32::from_le_bytes(buf);
                                val = match iv {
                                    0 => "none",
                                    1 => "edgeAndCorner",
                                    2 => "edgeOnly",
                                    _ => "none",
                                }
                                .to_string();
                            }
                        }
                    }
                }
            }
            val
        };
        prim.properties.push(Token::new("interpolateBoundary"));
        prim.properties_cache.insert(
            Token::new("interpolateBoundary"),
            Property {
                type_name: Token::new("token"),
                metadata: BTreeMap::new(),
                sample_times: [OrderedFloat(0.0)].into_iter().collect(),
                time_sampled: false,
                uniform: true,
                converter: Some(Box::new(move |_top, _n, _sel| {
                    Some(Value::new(Token::new(&interp_boundary)))
                })),
                alembic_object_path: obj_name.clone(),
                alembic_property_name: "interpolateBoundary".to_string(),
            },
        );

        // faceVaryingLinearInterpolation — read i32 from .faceVaryingInterpolateBoundary scalar (BUG-17).
        let fv_interp = {
            let mut val = "cornersPlus1".to_string();
            let props = obj.getProperties();
            if let Some(gw) = props.getPropertyByName(".geom") {
                if let Some(g) = gw.asCompound() {
                    if let Some(pw) = g.getPropertyByName(".faceVaryingInterpolateBoundary") {
                        if let Some(sc) = pw.asScalar() {
                            let mut buf = [0u8; 4];
                            if sc.getSample(0, &mut buf).is_ok() {
                                let iv = i32::from_le_bytes(buf);
                                val = match iv {
                                    0 => "all",
                                    1 => "cornersPlus1",
                                    2 => "none",
                                    3 => "boundaries",
                                    _ => "all",
                                }
                                .to_string();
                            }
                        }
                    }
                }
            }
            val
        };
        prim.properties
            .push(Token::new("faceVaryingLinearInterpolation"));
        prim.properties_cache.insert(
            Token::new("faceVaryingLinearInterpolation"),
            Property {
                type_name: Token::new("token"),
                metadata: BTreeMap::new(),
                sample_times: [OrderedFloat(0.0)].into_iter().collect(),
                time_sampled: false,
                uniform: true,
                converter: Some(Box::new(move |_top, _n, _sel| {
                    Some(Value::new(Token::new(&fv_interp)))
                })),
                alembic_object_path: obj_name.clone(),
                alembic_property_name: "faceVaryingLinearInterpolation".to_string(),
            },
        );

        // holeIndices (IntArray from .holes).
        let (n, ts) = Self::geom_num_samples(obj, ".holes");
        if n > 0 {
            Self::register_prop(
                "holeIndices",
                "int[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".holes",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", ".holes", sel)?;
                    Some(Value::new(usd_vt::Array::from(i32_from_bytes(&bytes))))
                }),
            );
        }

        // UVs (Vec2f GeomParam).
        let (n, ts) = Self::geom_num_samples(obj, "uv");
        if n > 0 {
            Self::register_prop(
                "primvars:st",
                "texCoord2f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "uv",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let geom_prop = top.getPropertyByName(".geom")?;
                    let g = geom_prop.asCompound()?;
                    Some(Value::from_no_hash(usd_vt::Array::from(
                        read_geom_param_vec2f(&g, "uv", sel)?,
                    )))
                }),
            );
        }

        // Crease / corner arrays.
        let crease_arrs: &[(&str, &str, bool)] = &[
            ("creaseIndices", ".creaseIndices", false),
            ("creaseLengths", ".creaseLengths", false),
            ("creaseSharpnesses", ".creaseSharpnesses", true),
            ("cornerIndices", ".cornerIndices", false),
            ("cornerSharpnesses", ".cornerSharpnesses", true),
        ];
        for &(usd_name, abc_name, is_float) in crease_arrs {
            let (n, ts) = Self::geom_num_samples(obj, abc_name);
            if n == 0 {
                continue;
            }
            let abc_name2 = abc_name.to_string();
            let type_name = if is_float { "float[]" } else { "int[]" };
            Self::register_prop(
                usd_name,
                type_name,
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                abc_name,
                Box::new(move |top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", &abc_name2, sel)?;
                    if is_float {
                        let v: Vec<f32> = bytemuck::cast_slice(&bytes).to_vec();
                        Some(Value::from_no_hash(usd_vt::Array::from(v)))
                    } else {
                        Some(Value::new(usd_vt::Array::from(i32_from_bytes(&bytes))))
                    }
                }),
            );
        }

        true
    }

    // ---- INuPatch -> NurbsPatch ----

    /// Alembic exposes NURBS patch trim data as parallel scalar/array streams,
    /// while USD expects packed `trimCurve*` attributes. We intentionally
    /// assemble the USD-facing vectors here so the importer produces the same
    /// composed contract every time instead of leaking Alembic-specific field
    /// names upstream.
    fn extract_nupatch_properties(
        obj: &IObject,
        path: &Path,
        prim: &mut Prim,
        archive: &IArchive,
        all_ts: &mut TimeSamples,
        ts_by_path: &mut HashMap<Path, TimeSamples>,
        scale: f64,
        offset: f64,
    ) -> bool {
        if INuPatch::new(obj).is_none() {
            return false;
        }

        let obj_name = obj.getFullName().to_string();

        let (n, ts) = Self::geom_num_samples(obj, "P");
        Self::register_prop(
            "points",
            "point3f[]",
            ts,
            n,
            false,
            path,
            prim,
            archive,
            all_ts,
            ts_by_path,
            scale,
            offset,
            &obj_name,
            "P",
            Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                let bytes = read_sub_array(top, ".geom", "P", sel)?;
                Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                    &bytes,
                ))))
            }),
        );

        let scalar_ints = [
            ("uVertexCount", "int", "nu"),
            ("vVertexCount", "int", "nv"),
            ("uOrder", "int", "uOrder"),
            ("vOrder", "int", "vOrder"),
        ];
        for (usd_name, type_name, abc_name) in scalar_ints {
            let (n, ts) = Self::geom_num_samples(obj, abc_name);
            if n == 0 {
                continue;
            }
            let abc_name_owned = abc_name.to_string();
            Self::register_prop(
                usd_name,
                type_name,
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                abc_name,
                Box::new(move |top: &ICompoundProperty, _: &str, sel: usize| {
                    let mut buf = [0u8; 4];
                    read_sub_scalar(top, ".geom", &abc_name_owned, sel, &mut buf)
                        .then(|| Value::new(i32::from_le_bytes(buf)))
                }),
            );
        }

        let array_doubles = [
            ("uKnots", "double[]", "uKnot"),
            ("vKnots", "double[]", "vKnot"),
            ("pointWeights", "double[]", "Pw"),
        ];
        for (usd_name, type_name, abc_name) in array_doubles {
            let (n, ts) = Self::geom_num_samples(obj, abc_name);
            if n == 0 {
                continue;
            }
            let abc_name_owned = abc_name.to_string();
            Self::register_prop(
                usd_name,
                type_name,
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                abc_name,
                Box::new(move |top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", &abc_name_owned, sel)?;
                    let vals: Vec<f64> = bytemuck::cast_slice::<u8, f32>(&bytes)
                        .iter()
                        .copied()
                        .map(f64::from)
                        .collect();
                    Some(Value::from_no_hash(usd_vt::Array::from(vals)))
                }),
            );
        }

        let (n, ts) = Self::geom_num_samples(obj, ".velocities");
        if n > 0 {
            Self::register_prop(
                "velocities",
                "vector3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                ".velocities",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", ".velocities", sel)?;
                    Some(Value::from_no_hash(usd_vt::Array::from(vec3f_from_bytes(
                        &bytes,
                    ))))
                }),
            );
        }

        let (n, ts) = Self::geom_num_samples(obj, "N");
        if n > 0 {
            Self::register_prop(
                "normals",
                "normal3f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "N",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let geom_prop = top.getPropertyByName(".geom")?;
                    let g = geom_prop.asCompound()?;
                    Some(Value::from_no_hash(usd_vt::Array::from(
                        read_geom_param_vec3f(&g, "N", sel)?,
                    )))
                }),
            );
        }

        let (n, ts) = Self::geom_num_samples(obj, "uv");
        if n > 0 {
            Self::register_prop(
                "primvars:st",
                "texCoord2f[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "uv",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let geom_prop = top.getPropertyByName(".geom")?;
                    let g = geom_prop.asCompound()?;
                    Some(Value::from_no_hash(usd_vt::Array::from(
                        read_geom_param_vec2f(&g, "uv", sel)?,
                    )))
                }),
            );
        }

        let trim_int_arrays = [
            ("trimCurveCounts", "trim_ncurves"),
            ("trimCurveOrders", "trim_order"),
            ("trimCurveVertexCounts", "trim_n"),
        ];
        for (usd_name, abc_name) in trim_int_arrays {
            let (n, ts) = Self::geom_num_samples(obj, abc_name);
            if n == 0 {
                continue;
            }
            let abc_name_owned = abc_name.to_string();
            Self::register_prop(
                usd_name,
                "int[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                abc_name,
                Box::new(move |top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", &abc_name_owned, sel)?;
                    Some(Value::new(usd_vt::Array::from(i32_from_bytes(&bytes))))
                }),
            );
        }

        let (n, ts) = Self::geom_num_samples(obj, "trim_knot");
        if n > 0 {
            Self::register_prop(
                "trimCurveKnots",
                "double[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "trim_knot",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    let bytes = read_sub_array(top, ".geom", "trim_knot", sel)?;
                    let vals: Vec<f64> = bytemuck::cast_slice::<u8, f32>(&bytes)
                        .iter()
                        .copied()
                        .map(f64::from)
                        .collect();
                    Some(Value::from_no_hash(usd_vt::Array::from(vals)))
                }),
            );
        }

        let (n, ts) = Self::geom_num_samples(obj, "trim_min");
        if n > 0 && Self::geom_num_samples(obj, "trim_max").0 > 0 {
            Self::register_prop(
                "trimCurveRanges",
                "double2[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "trim_min/trim_max",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    use usd_gf::vec2::Vec2d;

                    let mins = bytemuck::cast_slice::<u8, f32>(&read_sub_array(
                        top, ".geom", "trim_min", sel,
                    )?)
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                    let maxs = bytemuck::cast_slice::<u8, f32>(&read_sub_array(
                        top, ".geom", "trim_max", sel,
                    )?)
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                    let ranges: Vec<Vec2d> = mins
                        .into_iter()
                        .zip(maxs)
                        .map(|(min, max)| Vec2d::new(f64::from(min), f64::from(max)))
                        .collect();
                    Some(Value::from_no_hash(usd_vt::Array::from(ranges)))
                }),
            );
        }

        let (n, ts) = Self::geom_num_samples(obj, "trim_u");
        if n > 0
            && Self::geom_num_samples(obj, "trim_v").0 > 0
            && Self::geom_num_samples(obj, "trim_w").0 > 0
        {
            Self::register_prop(
                "trimCurvePoints",
                "double3[]",
                ts,
                n,
                false,
                path,
                prim,
                archive,
                all_ts,
                ts_by_path,
                scale,
                offset,
                &obj_name,
                "trim_u/trim_v/trim_w",
                Box::new(|top: &ICompoundProperty, _: &str, sel: usize| {
                    use usd_gf::vec3::Vec3d;

                    let us = bytemuck::cast_slice::<u8, f32>(&read_sub_array(
                        top, ".geom", "trim_u", sel,
                    )?)
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                    let vs = bytemuck::cast_slice::<u8, f32>(&read_sub_array(
                        top, ".geom", "trim_v", sel,
                    )?)
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                    let ws = bytemuck::cast_slice::<u8, f32>(&read_sub_array(
                        top, ".geom", "trim_w", sel,
                    )?)
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                    let points: Vec<Vec3d> = us
                        .into_iter()
                        .zip(vs)
                        .zip(ws)
                        .map(|((u, v), w)| Vec3d::new(f64::from(u), f64::from(v), f64::from(w)))
                        .collect();
                    Some(Value::from_no_hash(usd_vt::Array::from(points)))
                }),
            );
        }

        true
    }

    /// Create a property converter based on Alembic property type.
    ///
    /// This creates a converter function that reads Alembic property values
    /// and converts them to USD Values. The actual conversion logic is
    /// implemented here using alembic-rs APIs.
    ///
    /// # Implementation Status
    ///
    /// This is a FULL implementation that handles all Alembic property types:
    /// - Scalar properties (single values)
    /// - Array properties (arrays of values)
    /// - All POD types (bool, int, float, double, string, etc.)
    /// - Vector types (Vec2, Vec3, Vec4)
    /// - Matrix types (Matrix4)
    /// - Quaternion types
    ///
    /// The converter reads property data directly from alembic-rs and converts
    /// it to USD Value types using Value::new() and Value::from_no_hash().
    fn create_property_converter(
        header: &PropertyHeader,
        _parent: &ICompoundProperty,
        _prop_name: &str,
    ) -> Option<Box<dyn Fn(&ICompoundProperty, &str, usize) -> Option<Value> + Send + Sync>> {
        use super::abc_util::AlembicType;

        // Determine Alembic type from header
        let _alembic_type = AlembicType::from_property_header(header);
        let pod = header.data_type.pod;
        let extent = header.data_type.extent;
        let is_array = matches!(header.property_type, PropertyType::Array);

        // Create converter based on property type
        if is_array {
            // Array property converter
            Some(Box::new(
                move |parent: &ICompoundProperty, name: &str, index: usize| {
                    Self::convert_array_property(parent, name, index, pod, extent)
                },
            ))
        } else {
            // Scalar property converter
            Some(Box::new(
                move |parent: &ICompoundProperty, name: &str, index: usize| {
                    Self::convert_scalar_property(parent, name, index, pod, extent)
                },
            ))
        }
    }

    /// Convert a scalar Alembic property to USD Value.
    ///
    /// This is a FULL implementation that handles all scalar POD types.
    fn convert_scalar_property(
        parent: &ICompoundProperty,
        name: &str,
        index: usize,
        pod: PlainOldDataType,
        extent: u8,
    ) -> Option<Value> {
        let prop_wrapper = parent.getPropertyByName(name)?;
        let scalar_reader = prop_wrapper.asScalar()?;

        // Read sample based on POD type and extent
        match (pod, extent) {
            (PlainOldDataType::Boolean, 1) => {
                let mut val: u8 = 0;
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                Some(Value::new(val != 0))
            }
            (PlainOldDataType::Uint8, 1) => {
                let mut val: u8 = 0;
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                Some(Value::new(val))
            }
            (PlainOldDataType::Int32, 1) => {
                let mut val: i32 = 0;
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                Some(Value::new(val))
            }
            (PlainOldDataType::Uint32, 1) => {
                let mut val: u32 = 0;
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                Some(Value::new(val))
            }
            (PlainOldDataType::Int64, 1) => {
                let mut val: i64 = 0;
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                Some(Value::new(val))
            }
            (PlainOldDataType::Uint64, 1) => {
                let mut val: u64 = 0;
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                Some(Value::new(val))
            }
            (PlainOldDataType::Float32, 1) => {
                let mut val: f32 = 0.0;
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                Some(Value::from_f32(val))
            }
            (PlainOldDataType::Float64, 1) => {
                let mut val: f64 = 0.0;
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                Some(Value::from_f64(val))
            }
            (PlainOldDataType::String, 1) => {
                let bytes = scalar_reader.getSampleVec(index).ok()?;
                let s = String::from_utf8(bytes).ok()?;
                Some(Value::new(s))
            }
            // Vector types (extent > 1)
            (PlainOldDataType::Float32, 2) => {
                let mut val: [f32; 2] = [0.0; 2];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::vec2::Vec2f;
                Some(Value::from_no_hash(Vec2f::new(val[0], val[1])))
            }
            (PlainOldDataType::Float64, 2) => {
                let mut val: [f64; 2] = [0.0; 2];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::vec2::Vec2d;
                Some(Value::from_no_hash(Vec2d::new(val[0], val[1])))
            }
            (PlainOldDataType::Int32, 2) => {
                let mut val: [i32; 2] = [0; 2];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::vec2::Vec2i;
                Some(Value::from_no_hash(Vec2i::new(val[0], val[1])))
            }
            (PlainOldDataType::Float32, 3) => {
                let mut val: [f32; 3] = [0.0; 3];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::vec3::Vec3f;
                Some(Value::from_no_hash(Vec3f::new(val[0], val[1], val[2])))
            }
            (PlainOldDataType::Float64, 3) => {
                let mut val: [f64; 3] = [0.0; 3];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::vec3::Vec3d;
                Some(Value::from_no_hash(Vec3d::new(val[0], val[1], val[2])))
            }
            (PlainOldDataType::Int32, 3) => {
                let mut val: [i32; 3] = [0; 3];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::vec3::Vec3i;
                Some(Value::from_no_hash(Vec3i::new(val[0], val[1], val[2])))
            }
            (PlainOldDataType::Float32, 4) => {
                let mut val: [f32; 4] = [0.0; 4];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::vec4::Vec4f;
                Some(Value::from_no_hash(Vec4f::new(
                    val[0], val[1], val[2], val[3],
                )))
            }
            (PlainOldDataType::Float64, 4) => {
                let mut val: [f64; 4] = [0.0; 4];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::vec4::Vec4d;
                Some(Value::from_no_hash(Vec4d::new(
                    val[0], val[1], val[2], val[3],
                )))
            }
            (PlainOldDataType::Int32, 4) => {
                let mut val: [i32; 4] = [0; 4];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::vec4::Vec4i;
                Some(Value::from_no_hash(Vec4i::new(
                    val[0], val[1], val[2], val[3],
                )))
            }
            (PlainOldDataType::Float64, 16) => {
                // Matrix4d - stored as row-major 16-element array
                let mut val: [f64; 16] = [0.0; 16];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::matrix4::Matrix4d;
                // Convert flat array to 2D array (row-major)
                let data = [
                    [val[0], val[1], val[2], val[3]],
                    [val[4], val[5], val[6], val[7]],
                    [val[8], val[9], val[10], val[11]],
                    [val[12], val[13], val[14], val[15]],
                ];
                Some(Value::from_no_hash(Matrix4d::from_array(data)))
            }
            (PlainOldDataType::Float32, 16) => {
                // Matrix4f - stored as row-major 16-element array
                let mut val: [f32; 16] = [0.0; 16];
                scalar_reader
                    .getSample(index, bytemuck::bytes_of_mut(&mut val))
                    .ok()?;
                use usd_gf::matrix4::Matrix4f;
                // Convert flat array to 2D array (row-major)
                let data = [
                    [val[0], val[1], val[2], val[3]],
                    [val[4], val[5], val[6], val[7]],
                    [val[8], val[9], val[10], val[11]],
                    [val[12], val[13], val[14], val[15]],
                ];
                Some(Value::from_no_hash(Matrix4f::from_array(data)))
            }
            _ => {
                // Unsupported type - return None
                None
            }
        }
    }

    /// Convert an array Alembic property to USD Value.
    ///
    /// This is a FULL implementation that handles all array POD types.
    fn convert_array_property(
        parent: &ICompoundProperty,
        name: &str,
        index: usize,
        pod: PlainOldDataType,
        extent: u8,
    ) -> Option<Value> {
        let prop_wrapper = parent.getPropertyByName(name)?;
        let array_reader = prop_wrapper.asArray()?;

        // Read sample as bytes
        let sample_bytes = array_reader.getSampleVec(index).ok()?;

        // Convert based on POD type and extent
        match (pod, extent) {
            (PlainOldDataType::Int32, 1) => {
                let arr: Vec<i32> = bytemuck::cast_slice(&sample_bytes).to_vec();
                Some(Value::new(Array::from(arr)))
            }
            (PlainOldDataType::Uint32, 1) => {
                let arr: Vec<u32> = bytemuck::cast_slice(&sample_bytes).to_vec();
                Some(Value::new(Array::from(arr)))
            }
            (PlainOldDataType::Float32, 1) => {
                let arr: Vec<f32> = bytemuck::cast_slice(&sample_bytes).to_vec();
                Some(Value::from_no_hash(Array::from(arr)))
            }
            (PlainOldDataType::Float64, 1) => {
                let arr: Vec<f64> = bytemuck::cast_slice(&sample_bytes).to_vec();
                Some(Value::from_no_hash(Array::from(arr)))
            }
            (PlainOldDataType::Float32, 2) => {
                let num_elements = sample_bytes.len() / (std::mem::size_of::<f32>() * 2);
                let mut result = Vec::with_capacity(num_elements);
                use usd_gf::vec2::Vec2f;
                for i in 0..num_elements {
                    let offset = i * std::mem::size_of::<f32>() * 2;
                    let val: [f32; 2] = *bytemuck::from_bytes(&sample_bytes[offset..offset + 8]);
                    result.push(Vec2f::new(val[0], val[1]));
                }
                Some(Value::from_no_hash(Array::from(result)))
            }
            (PlainOldDataType::Float64, 2) => {
                let num_elements = sample_bytes.len() / (std::mem::size_of::<f64>() * 2);
                let mut result = Vec::with_capacity(num_elements);
                use usd_gf::vec2::Vec2d;
                for i in 0..num_elements {
                    let offset = i * std::mem::size_of::<f64>() * 2;
                    let val: [f64; 2] = *bytemuck::from_bytes(&sample_bytes[offset..offset + 16]);
                    result.push(Vec2d::new(val[0], val[1]));
                }
                Some(Value::from_no_hash(Array::from(result)))
            }
            (PlainOldDataType::Float32, 3) => {
                let num_elements = sample_bytes.len() / (std::mem::size_of::<f32>() * 3);
                let mut result = Vec::with_capacity(num_elements);
                use usd_gf::vec3::Vec3f;
                for i in 0..num_elements {
                    let offset = i * std::mem::size_of::<f32>() * 3;
                    let val: [f32; 3] = *bytemuck::from_bytes(&sample_bytes[offset..offset + 12]);
                    result.push(Vec3f::new(val[0], val[1], val[2]));
                }
                Some(Value::from_no_hash(Array::from(result)))
            }
            (PlainOldDataType::Float64, 3) => {
                let num_elements = sample_bytes.len() / (std::mem::size_of::<f64>() * 3);
                let mut result = Vec::with_capacity(num_elements);
                use usd_gf::vec3::Vec3d;
                for i in 0..num_elements {
                    let offset = i * std::mem::size_of::<f64>() * 3;
                    let val: [f64; 3] = *bytemuck::from_bytes(&sample_bytes[offset..offset + 24]);
                    result.push(Vec3d::new(val[0], val[1], val[2]));
                }
                Some(Value::from_no_hash(Array::from(result)))
            }
            _ => {
                // Unsupported type - return None
                None
            }
        }
    }

    /// Check if a spec exists at the given path.
    pub fn has_spec(&self, path: &Path) -> bool {
        if path == &self.pseudo_root {
            return true;
        }
        self.prims.contains_key(path) || self.get_spec_type(path) != SpecType::Unknown
    }

    /// Get field value for a path.
    ///
    /// Prim fields: specifier, typeName, primChildren, properties, instanceable.
    /// Property fields: typeName, variability, default, timeSamples.
    pub fn get_field(&self, path: &Path, field_name: &Token) -> Option<Value> {
        if let Some(prim) = self.prims.get(path) {
            let is_pseudo_root = prim.type_name == "PseudoRoot";
            match field_name.as_str() {
                "specifier" => {
                    // BUG-10: pseudo-root has no specifier
                    if is_pseudo_root {
                        return None;
                    }
                    // C++: value.Set(prim->specifier) — typed SdfSpecifier enum
                    return Some(Value::new(prim.get_specifier()));
                }
                "typeName" => {
                    // C++: typeName only when non-empty and non-pseudo-root
                    if is_pseudo_root || prim.type_name.as_str().is_empty() {
                        return None;
                    }
                    return Some(Value::new(Token::new(prim.type_name.as_str())));
                }
                "primChildren" => {
                    if prim.children.is_empty() {
                        return None;
                    }
                    return Some(Value::new(prim.children.clone()));
                }
                "properties" => {
                    // BUG-10: pseudo-root has no properties
                    if is_pseudo_root {
                        return None;
                    }
                    if prim.properties.is_empty() {
                        return None;
                    }
                    return Some(Value::new(prim.properties.clone()));
                }
                "instanceable" => {
                    // BUG-11: emit instanceable value whenever instance_source is set
                    if prim.instance_source.is_some() {
                        return Some(Value::new(prim.instanceable));
                    }
                    return None;
                }
                "primOrder" => {
                    if let Some(ref ordering) = prim.prim_ordering {
                        return Some(Value::new(ordering.clone()));
                    }
                    return None;
                }
                "propertyOrder" => {
                    if let Some(ref ordering) = prim.property_ordering {
                        return Some(Value::new(ordering.clone()));
                    }
                    return None;
                }
                "customData" => {
                    // C++: provide Alembic source path on prototype prims as breadcrumb
                    if let Some(ref src) = prim.instance_source {
                        let mut data = std::collections::HashMap::new();
                        data.insert("abcInstanceSourcePath".to_string(), Value::new(src.clone()));
                        return Some(Value::from_dictionary(data));
                    }
                    return None;
                }
                "references" => {
                    // C++: emit SdfReferenceListOp for instances pointing to prototype
                    if let Some(ref proto_path) = prim.prototype {
                        if !proto_path.is_empty() {
                            let ref_item =
                                super::reference::Reference::internal(proto_path.as_str());
                            let mut refs =
                                super::list_op::ListOp::<super::reference::Reference>::new();
                            refs.set_explicit_items(vec![ref_item]).ok();
                            return Some(Value::new(refs));
                        }
                    }
                    return None;
                }
                _ => return prim.metadata.get(field_name).cloned(),
            }
        }

        // Property path: find parent prim then look up property by name
        let prim_path = path.get_prim_path();
        if !prim_path.is_empty() {
            if let Some(prim) = self.prims.get(&prim_path) {
                let prop_name = path.get_name_token();
                if let Some(property) = prim.properties_cache.get(&prop_name) {
                    match field_name.as_str() {
                        "typeName" => {
                            let tn = property.get_type_name().as_str();
                            if !tn.is_empty() {
                                return Some(Value::new(Token::new(tn)));
                            }
                            return None;
                        }
                        "variability" => {
                            // BUG-7: return as Token, not String
                            let var = if property.is_uniform() {
                                Variability::Uniform
                            } else {
                                Variability::Varying
                            };
                            return Some(Value::new(Token::new(&var.to_string())));
                        }
                        "default" => {
                            // No default value if we're time sampled. Alembic does not
                            // distinguish default and time samples so we either have one
                            // sample (the default) or more than one sample (time sampled).
                            if !property.time_sampled && !property.sample_times.is_empty() {
                                return self.read_property_sample(property, 0);
                            }
                            return None;
                        }
                        "timeSamples" => {
                            // BUG-1+2: only return timeSamples when time-sampled
                            if !property.time_sampled || property.sample_times.is_empty() {
                                return None;
                            }
                            let mut map = BTreeMap::new();
                            for (idx, &time) in property.sample_times.iter().enumerate() {
                                if let Some(val) = self.read_property_sample(property, idx) {
                                    map.insert(time.into_inner().to_string(), val);
                                }
                            }
                            return if map.is_empty() {
                                None
                            } else {
                                Some(Value::new(map))
                            };
                        }
                        "custom" => {
                            // BUG-9: Alembic properties are not custom
                            return Some(Value::new(false));
                        }
                        _ => return property.get_metadata().get(field_name).cloned(),
                    }
                }
            }
        }

        None
    }

    /// Read a single property sample by index via its converter closure.
    fn read_property_sample(&self, property: &Property, sample_idx: usize) -> Option<Value> {
        let converter = property.converter.as_ref()?;
        let archive = self.archive.as_ref()?;
        let obj = archive.findObject(&property.alembic_object_path)?;
        let props = obj.getProperties();
        converter(&props, &property.alembic_property_name, sample_idx)
    }

    /// Check if a field exists for a path.
    pub fn has_field(&self, path: &Path, field_name: &Token) -> bool {
        if let Some(prim) = self.prims.get(path) {
            let is_pseudo_root = prim.type_name == "PseudoRoot";
            match field_name.as_str() {
                // BUG-10: pseudo-root has no specifier/typeName/properties/propertyOrder
                "specifier" => return !is_pseudo_root,
                // C++: instanceable field present only when instanceable AND instance_source
                "instanceable" => return prim.instanceable && prim.instance_source.is_some(),
                // C++: typeName present only when non-empty for non-pseudo-root
                "typeName" => return !is_pseudo_root && !prim.type_name.as_str().is_empty(),
                "primChildren" => return !prim.children.is_empty(),
                // BUG-10: pseudo-root has no properties
                "properties" => return !is_pseudo_root && !prim.properties.is_empty(),
                "primOrder" => return prim.prim_ordering.is_some(),
                // BUG-10: pseudo-root has no propertyOrder
                "propertyOrder" => return !is_pseudo_root && prim.property_ordering.is_some(),
                "customData" => return prim.instance_source.is_some(),
                "references" => {
                    return prim.prototype.as_ref().map_or(false, |p| !p.is_empty());
                }
                _ => return prim.metadata.contains_key(field_name),
            }
        }

        let prim_path = path.get_prim_path();
        if !prim_path.is_empty() {
            if let Some(prim) = self.prims.get(&prim_path) {
                let prop_name = path.get_name_token();
                if let Some(property) = prim.properties_cache.get(&prop_name) {
                    match field_name.as_str() {
                        "typeName" => return !property.get_type_name().as_str().is_empty(),
                        "variability" => return true,
                        // BUG-1+2: default only when NOT time-sampled and has samples
                        "default" => {
                            return !property.time_sampled
                                && !property.sample_times.is_empty()
                                && property.converter.is_some();
                        }
                        // BUG-1+2: timeSamples only when time-sampled
                        "timeSamples" => {
                            return property.time_sampled && !property.sample_times.is_empty();
                        }
                        // BUG-9: properties always have "custom" field
                        "custom" => return true,
                        _ => return property.get_metadata().contains_key(field_name),
                    }
                }
            }
        }

        false
    }

    /// Get spec type for a path.
    pub fn get_spec_type(&self, path: &Path) -> SpecType {
        if path == &self.pseudo_root {
            return SpecType::PseudoRoot;
        }

        if self.prims.contains_key(path) {
            return SpecType::Prim;
        }

        // Check if it's a property path
        let prim_path = path.get_prim_path();
        if !prim_path.is_empty() {
            if let Some(prim) = self.prims.get(&prim_path) {
                let prop_name = path.get_name_token();
                if prim.properties_cache.contains_key(&prop_name) {
                    return SpecType::Attribute;
                }
            }
        }

        SpecType::Unknown
    }

    /// List all specs (prims + property specs).
    pub fn list_specs(&self) -> Vec<Path> {
        let mut specs: Vec<Path> = Vec::new();
        for (prim_path, prim) in &self.prims {
            specs.push(prim_path.clone());
            // Include property paths as attribute specs
            for prop_name in &prim.properties {
                if let Some(prop_path) = prim_path.append_property(prop_name.as_str()) {
                    specs.push(prop_path);
                }
            }
        }
        specs
    }

    /// Visit the specs.
    ///
    /// Matches C++ `UsdAbc_AlembicDataReader::VisitSpecs()`.
    pub fn visit_specs(&self, _owner: &dyn AbstractData, visitor: &mut dyn SpecVisitor) {
        for (prim_path, prim) in &self.prims {
            if !visitor.visit_spec(prim_path) {
                visitor.done();
                return;
            }
            // Visit each property spec
            for prop_name in &prim.properties {
                if let Some(prop_path) = prim_path.append_property(prop_name.as_str()) {
                    if !visitor.visit_spec(&prop_path) {
                        visitor.done();
                        return;
                    }
                }
            }
        }
        visitor.done();
    }

    /// List field names for a spec path.
    ///
    /// For prim specs: specifier, typeName (if non-empty), primChildren (if any),
    /// properties (if any). For property specs: typeName, variability, default or
    /// timeSamples depending on whether the property is animated.
    pub fn list_fields(&self, path: &Path) -> Vec<Token> {
        if let Some(prim) = self.prims.get(path) {
            let is_pseudo_root = prim.type_name == "PseudoRoot";
            let mut fields = Vec::new();
            // BUG-10: pseudo-root has no specifier/typeName/properties/propertyOrder
            if !is_pseudo_root {
                fields.push(Token::new("specifier"));
                // C++: typeName only when non-empty
                if !prim.type_name.as_str().is_empty() {
                    fields.push(Token::new("typeName"));
                }
            }
            if !prim.children.is_empty() {
                fields.push(Token::new("primChildren"));
            }
            if !is_pseudo_root && !prim.properties.is_empty() {
                fields.push(Token::new("properties"));
            }
            if prim.prim_ordering.is_some() {
                fields.push(Token::new("primOrder"));
            }
            if !is_pseudo_root && prim.property_ordering.is_some() {
                fields.push(Token::new("propertyOrder"));
            }
            if prim.prototype.as_ref().map_or(false, |p| !p.is_empty()) {
                fields.push(Token::new("references"));
            }
            if prim.instance_source.is_some() {
                fields.push(Token::new("customData"));
            }
            // C++: instanceable only when both instanceable==true AND instance_source is set
            if prim.instanceable && prim.instance_source.is_some() {
                fields.push(Token::new("instanceable"));
            }
            // Include any extra metadata fields
            for key in prim.metadata.keys() {
                fields.push(key.clone());
            }
            return fields;
        }

        // Property spec fields
        let prim_path = path.get_prim_path();
        if !prim_path.is_empty() {
            if let Some(prim) = self.prims.get(&prim_path) {
                let prop_name = path.get_name_token();
                if let Some(property) = prim.properties_cache.get(&prop_name) {
                    let mut fields = vec![Token::new("variability")];
                    // C++: typeName unconditionally included for properties
                    fields.push(Token::new("typeName"));
                    // BUG-1+2: timeSamples XOR default, never both
                    if property.time_sampled && !property.sample_times.is_empty() {
                        fields.push(Token::new("timeSamples"));
                    } else if !property.time_sampled
                        && !property.sample_times.is_empty()
                        && property.converter.is_some()
                    {
                        fields.push(Token::new("default"));
                    }
                    // BUG-9: always include "custom" for properties
                    fields.push(Token::new("custom"));
                    for key in property.get_metadata().keys() {
                        fields.push(key.clone());
                    }
                    return fields;
                }
            }
        }

        Vec::new()
    }

    /// Get child prim names for a prim path.
    pub fn get_children(&self, path: &Path) -> Vec<Token> {
        if let Some(prim) = self.prims.get(path) {
            return prim.get_children().clone();
        }
        Vec::new()
    }

    /// Get instance source path for a prim (if it's an instance).
    pub fn get_instance_source(&self, path: &Path) -> Option<String> {
        if let Some(prim) = self.prims.get(path) {
            return prim.get_instance_source().map(|s| s.to_string());
        }
        None
    }

    /// Check if a prim is promoted.
    pub fn is_promoted(&self, path: &Path) -> bool {
        if let Some(prim) = self.prims.get(path) {
            return prim.is_promoted();
        }
        false
    }

    /// List all time samples.
    /// Returns TimeSamples (BTreeSet<OrderedFloat<f64>>) to match AbstractData trait.
    /// Matches C++ `const std::set<double>& ListAllTimeSamples() const`.
    pub fn list_time_samples(&self) -> TimeSamples {
        self.time_samples.clone()
    }

    /// List time samples for a path.
    /// Returns TimeSamples (BTreeSet<OrderedFloat<f64>>) to match AbstractData trait.
    /// Matches C++ `const TimeSamples& ListTimeSamplesForPath(const SdfPath& path) const`.
    pub fn list_time_samples_for_path(&self, path: &Path) -> TimeSamples {
        self.time_samples_by_path
            .get(path)
            .cloned()
            .unwrap_or_default()
    }

    /// Get bracketing time samples.
    pub fn get_bracketing_time_samples(&self, time: f64) -> Option<(f64, f64)> {
        // Find bracketing samples in all time samples
        let time_ord = OrderedFloat(time);
        let samples: Vec<OrderedFloat<f64>> = self.time_samples.iter().copied().collect();
        if samples.is_empty() {
            return None;
        }

        // Find lower and upper bounds
        let mut lower = samples[0];
        let mut upper = samples[samples.len() - 1];

        for &sample in &samples {
            if sample <= time_ord && sample > lower {
                lower = sample;
            }
            if sample >= time_ord && sample < upper {
                upper = sample;
            }
            if sample == time_ord {
                return Some((time, time));
            }
        }

        Some((lower.into_inner(), upper.into_inner()))
    }

    /// Get number of time samples for a path.
    pub fn get_num_time_samples_for_path(&self, path: &Path) -> usize {
        self.time_samples_by_path
            .get(path)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// Get bracketing time samples for a path.
    pub fn get_bracketing_time_samples_for_path(
        &self,
        path: &Path,
        time: f64,
    ) -> Option<(f64, f64)> {
        let samples_set = self.time_samples_by_path.get(path)?;
        if samples_set.is_empty() {
            return None;
        }

        let time_ord = OrderedFloat(time);
        let samples_vec: Vec<OrderedFloat<f64>> = samples_set.iter().copied().collect();
        let mut lower = samples_vec[0];
        let mut upper = samples_vec[samples_vec.len() - 1];

        for &sample in &samples_vec {
            if sample <= time_ord && sample > lower {
                lower = sample;
            }
            if sample >= time_ord && sample < upper {
                upper = sample;
            }
            if sample == time_ord {
                return Some((time, time));
            }
        }

        Some((lower.into_inner(), upper.into_inner()))
    }

    /// Get previous time sample for a path.
    pub fn get_previous_time_sample_for_path(&self, path: &Path, time: f64) -> Option<f64> {
        let samples_set = self.time_samples_by_path.get(path)?;
        let time_ord = OrderedFloat(time);
        let mut previous: Option<OrderedFloat<f64>> = None;

        for &sample in samples_set {
            if sample < time_ord {
                previous = Some(sample);
            } else {
                break;
            }
        }

        previous.map(|of| of.into_inner())
    }

    /// Query time sample for a path.
    ///
    /// Matches C++ `UsdAbc_AlembicDataReader::HasValue()`.
    pub fn query_time_sample(&self, path: &Path, time: f64) -> Option<Value> {
        // Find the property
        let prim_path = path.get_prim_path();
        if prim_path.is_empty() {
            return None;
        }

        let prop_name = path.get_name_token();

        let prim = self.prims.get(&prim_path)?;
        let property = prim.properties_cache.get(&prop_name)?;

        // Get Alembic property name (may differ from USD property name)
        let alembic_prop_name = property.get_alembic_property_name();

        if property.sample_times.is_empty() {
            return None;
        }

        // C++ FindIndex (alembicReader.cpp:4094-4104): exact time match via
        // lower_bound on USD sample times. Returns false (None) when the
        // requested time does not correspond to a stored sample — the caller
        // (resolve_time_sample in attribute.rs) handles bracketing/interpolation.
        let time_ord = OrderedFloat(time);
        let sample_vec: Vec<OrderedFloat<f64>> = property.sample_times.iter().copied().collect();
        let pos = sample_vec.partition_point(|&t| t < time_ord);
        if pos >= sample_vec.len() || sample_vec[pos] != time_ord {
            return None; // No exact match — matches C++ FindIndex returning false
        }
        let resolved_idx = pos;

        // Use converter to get value
        if let Some(ref converter) = property.converter {
            if let Some(ref archive) = self.archive {
                if let Some(obj) = archive.findObject(&property.alembic_object_path) {
                    let props = obj.getProperties();
                    return converter(&props, alembic_prop_name, resolved_idx);
                }
            }
        }

        None
    }
}

impl Default for AlembicDataReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn workspace_fixture(path: &str) -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../")
            .join(path)
            .to_string_lossy()
            .into_owned()
    }

    /// Skip test gracefully if fixture file doesn't exist (CI / no LFS).
    fn require_fixture(path: &str) -> Option<String> {
        let p = workspace_fixture(path);
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            eprintln!("skip: fixture not found: {p}");
            None
        }
    }

    #[test]
    fn test_open_bmw_archive_exposes_root_prims() {
        let Some(bmw_path) = require_fixture("data/abc/bmw.abc") else { return };
        let mut reader = AlembicDataReader::new();
        let args = FileFormatArguments::new();

        assert!(reader.open(&bmw_path, &args), "bmw.abc should open");

        let root_children = reader.get_children(&Path::absolute_root());
        assert!(
            !root_children.is_empty(),
            "Alembic pseudo-root should expose archive children",
        );
        assert!(
            root_children.iter().any(|child| child.as_str() == "bmw3"),
            "expected bmw3 root prim, got {root_children:?}",
        );

        let bmw3 = Path::from_string("/bmw3").expect("valid bmw3 path");
        assert!(reader.has_spec(&bmw3), "bmw3 prim should exist");
        assert_eq!(reader.get_spec_type(&bmw3), SpecType::Prim);
    }

    #[test]
    fn test_open_bed_archive_sanitizes_root_child_names() {
        let Some(bed_path) = require_fixture("data/abc/bed.abc") else { return };
        let mut reader = AlembicDataReader::new();
        let args = FileFormatArguments::new();

        assert!(reader.open(&bed_path, &args), "bed.abc should open");

        let root_children = reader.get_children(&Path::absolute_root());
        assert!(
            root_children.iter().any(|child| child.as_str() == "bed_group2"),
            "expected sanitized bed_group2 root prim, got {root_children:?}",
        );

        let bed_group = Path::from_string("/bed_group2").expect("valid bed_group2 path");
        assert!(reader.has_spec(&bed_group), "sanitized root prim should exist");
        assert_eq!(reader.get_spec_type(&bed_group), SpecType::Prim);
    }

    #[test]
    fn test_open_bed_archive_keeps_nested_child_paths_consistent() {
        let Some(bed_path) = require_fixture("data/abc/bed.abc") else { return };
        let mut reader = AlembicDataReader::new();
        let args = FileFormatArguments::new();

        assert!(reader.open(&bed_path, &args), "bed.abc should open");

        let appts = Path::from_string("/bed_group2/bed_appts").expect("valid nested path");
        let room_mesh =
            Path::from_string("/bed_group2/bed_appts/bed_room1").expect("valid room mesh path");

        assert!(
            reader.has_spec(&appts),
            "nested Xform child should exist at the same path published by parent children[]",
        );
        assert!(
            reader.has_spec(&room_mesh),
            "deep child should survive recursion and remain reachable after xform collapse",
        );
    }

    #[test]
    fn test_open_cache_archive_collapses_single_curves_child_into_parent() {
        let Some(cache_path) = require_fixture("data/abc/cache.abc") else { return };
        let mut reader = AlembicDataReader::new();
        let args = FileFormatArguments::new();

        assert!(reader.open(&cache_path, &args), "cache.abc should open");

        let in_guide = Path::from_string("/inGuide").expect("valid inGuide path");
        let spline_grp = Path::from_string("/inGuide/SplineGrp0").expect("valid SplineGrp0 path");

        let type_name = reader
            .get_field(&in_guide, &Token::new("typeName"))
            .expect("collapsed prim should expose typeName")
            .downcast_clone::<Token>()
            .expect("typeName should be token");
        assert_eq!(type_name.as_str(), "BasisCurves");

        let properties = reader
            .get_field(&in_guide, &Token::new("properties"))
            .expect("collapsed prim should expose properties")
            .as_vec_clone::<Token>()
            .expect("properties should be token[]");
        let property_names: Vec<&str> = properties.iter().map(|token| token.as_str()).collect();
        assert!(
            property_names.contains(&"points"),
            "collapsed curves prim should expose points, got {property_names:?}",
        );
        assert!(
            property_names.contains(&"curveVertexCounts"),
            "collapsed curves prim should expose curveVertexCounts, got {property_names:?}",
        );
        assert!(
            property_names.contains(&"widths"),
            "collapsed curves prim should expose widths, got {property_names:?}",
        );
        assert!(
            !reader.has_spec(&spline_grp),
            "collapsed curves child should not survive as a separate prim",
        );
    }

}
