/// Patch type identifiers mirroring Far::PatchDescriptor::Type.
///
/// Numeric values match C++ OpenSubdiv 3.7.0 enum exactly.
/// `Ord`/`PartialOrd` follow the C++ `operator<` which compares by integer value,
/// enabling use as `BTreeMap` keys and sorted collections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i32)]
pub enum PatchType {
    NonPatch = 0,
    Points = 1,
    Lines = 2,
    Quads = 3,
    Triangles = 4,
    Loop = 5,
    Regular = 6,
    Gregory = 7,
    GregoryBoundary = 8,
    GregoryBasis = 9,
    GregoryTriangle = 10,
}

impl PatchType {
    /// Number of control vertices for this patch type.
    ///
    /// Mirrors C++ `PatchDescriptor::GetNumControlVertices(Type)`.
    #[doc(alias = "GetNumControlVertices")]
    pub fn num_control_vertices(self) -> i32 {
        match self {
            // C++ returns -1 for NON_PATCH (switch default branch)
            PatchType::NonPatch => -1,
            PatchType::Points => 1,
            PatchType::Lines => 2,
            PatchType::Quads => 4,
            PatchType::Triangles => 3,
            PatchType::Loop => 12,
            PatchType::Regular => 16,
            // Gregory and GregoryBoundary both use the 4-CV legacy layout
            PatchType::Gregory => 4,
            PatchType::GregoryBoundary => 4,
            PatchType::GregoryBasis => 20,
            PatchType::GregoryTriangle => 18,
        }
    }

    /// Returns the integer discriminant (mirrors C++ GetType()).
    pub fn get_type(self) -> i32 {
        self as i32
    }

    /// True for any adaptive (non-linear) patch type.
    ///
    /// Mirrors C++ `PatchDescriptor::IsAdaptive(Type)`: returns true when
    /// `type > TRIANGLES`.
    #[doc(alias = "IsAdaptive")]
    pub fn is_adaptive(self) -> bool {
        (self as i32) > (PatchType::Triangles as i32)
    }

    /// Number of control vertices of Regular patches (16).
    pub const fn regular_patch_size() -> i32 {
        16
    }

    /// Number of control vertices of Gregory / GregoryBoundary patches (4).
    pub const fn gregory_patch_size() -> i32 {
        4
    }

    /// Number of control vertices of GregoryBasis patches (20).
    pub const fn gregory_basis_patch_size() -> i32 {
        20
    }
}

impl Default for PatchType {
    fn default() -> Self {
        PatchType::NonPatch
    }
}

/// Describes a set of patches: type + associated CV count.
///
/// Mirrors C++ `Far::PatchDescriptor`.
/// `Ord`/`PartialOrd` mirror C++ `operator<` which delegates to the underlying
/// `Type` integer, making `PatchDescriptor` usable as a `BTreeMap` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct PatchDescriptor {
    pub patch_type: PatchType,
}

impl PatchDescriptor {
    pub fn new(patch_type: PatchType) -> Self {
        Self { patch_type }
    }

    /// Returns the patch type integer (mirrors C++ `GetType()`).
    #[doc(alias = "GetType")]
    pub fn get_type(&self) -> i32 {
        self.patch_type as i32
    }

    /// Returns the number of control vertices for this patch type.
    #[doc(alias = "GetNumControlVertices")]
    pub fn get_num_control_vertices(&self) -> i32 {
        self.patch_type.num_control_vertices()
    }

    /// Deprecated: same as `get_num_control_vertices`.
    ///
    /// Mirrors C++ `GetNumFVarControlVertices()`.
    pub fn get_num_f_var_control_vertices(&self) -> i32 {
        self.get_num_control_vertices()
    }

    /// True if this is an adaptive (non-linear) patch.
    pub fn is_adaptive(&self) -> bool {
        self.patch_type.is_adaptive()
    }

    /// Returns the list of valid adaptive patch descriptors for a given
    /// subdivision scheme.
    ///
    /// Mirrors C++ `PatchDescriptor::GetAdaptivePatchDescriptors(SchemeType)`.
    #[doc(alias = "GetAdaptivePatchDescriptors")]
    pub fn get_adaptive_patch_descriptors(
        scheme: crate::sdc::types::SchemeType,
    ) -> &'static [PatchDescriptor] {
        use crate::sdc::types::SchemeType;
        // Static tables, matching C++ patchDescriptor.cpp
        static CATMARK: &[PatchDescriptor] = &[
            PatchDescriptor {
                patch_type: PatchType::Regular,
            },
            PatchDescriptor {
                patch_type: PatchType::Gregory,
            },
            PatchDescriptor {
                patch_type: PatchType::GregoryBoundary,
            },
            PatchDescriptor {
                patch_type: PatchType::GregoryBasis,
            },
        ];
        static LOOP: &[PatchDescriptor] = &[
            PatchDescriptor {
                patch_type: PatchType::Loop,
            },
            PatchDescriptor {
                patch_type: PatchType::GregoryTriangle,
            },
        ];
        static BILINEAR: &[PatchDescriptor] = &[];
        match scheme {
            SchemeType::Bilinear => BILINEAR,
            SchemeType::Catmark => CATMARK,
            SchemeType::Loop => LOOP,
        }
    }
}
