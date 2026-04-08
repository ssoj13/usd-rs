// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 sdc/types.h + sdc/typeTraits.cpp

/// All subdivision schemes supported by OpenSubdiv.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SchemeType {
    Bilinear = 0,
    Catmark = 1,
    Loop = 2,
}

/// Face splitting strategy used by each scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Split {
    /// Used by Catmark and Bilinear.
    ToQuads,
    /// Used by Loop.
    ToTris,
    /// Not currently used (potential future extension).
    Hybrid,
}

/// Static trait table entry, mirroring the C++ `TraitsEntry` struct.
struct TraitsEntry {
    name: &'static str,
    split_type: Split,
    regular_face_size: i32,
    regular_vertex_valence: i32,
    local_neighborhood: i32,
}

// The three rows correspond to Bilinear=0, Catmark=1, Loop=2.
static TRAITS_TABLE: [TraitsEntry; 3] = [
    TraitsEntry {
        name: "bilinear",
        split_type: Split::ToQuads,
        regular_face_size: 4,
        regular_vertex_valence: 4,
        local_neighborhood: 0,
    },
    TraitsEntry {
        name: "catmark",
        split_type: Split::ToQuads,
        regular_face_size: 4,
        regular_vertex_valence: 4,
        local_neighborhood: 1,
    },
    TraitsEntry {
        name: "loop",
        split_type: Split::ToTris,
        regular_face_size: 3,
        regular_vertex_valence: 6,
        local_neighborhood: 1,
    },
];

/// Traits associated with all subdivision scheme types, indexed by `SchemeType`.
///
/// Mirrors the C++ `Sdc::SchemeTypeTraits` class.
pub struct SchemeTypeTraits;

impl SchemeTypeTraits {
    /// Return the enum value itself (identity, mirrors C++ `GetType`).
    #[inline]
    pub fn get_type(scheme_type: SchemeType) -> SchemeType {
        scheme_type
    }

    /// Return how the scheme splits faces topologically.
    #[inline]
    pub fn get_topological_split_type(scheme_type: SchemeType) -> Split {
        TRAITS_TABLE[scheme_type as usize].split_type
    }

    /// Return the number of vertices per regular face for this scheme.
    #[inline]
    pub fn get_regular_face_size(scheme_type: SchemeType) -> i32 {
        TRAITS_TABLE[scheme_type as usize].regular_face_size
    }

    /// Return the regular vertex valence for this scheme.
    #[inline]
    pub fn get_regular_vertex_valence(scheme_type: SchemeType) -> i32 {
        TRAITS_TABLE[scheme_type as usize].regular_vertex_valence
    }

    /// Return the local neighborhood size (rings) for this scheme.
    #[inline]
    pub fn get_local_neighborhood_size(scheme_type: SchemeType) -> i32 {
        TRAITS_TABLE[scheme_type as usize].local_neighborhood
    }

    /// Return the human-readable name of this scheme.
    #[inline]
    pub fn get_name(scheme_type: SchemeType) -> &'static str {
        TRAITS_TABLE[scheme_type as usize].name
    }

    // ── Short-name aliases (for callers that omit the `get_` prefix) ─────────

    #[inline]
    pub fn regular_face_size(s: SchemeType) -> i32 {
        Self::get_regular_face_size(s)
    }
    #[inline]
    pub fn regular_vertex_valence(s: SchemeType) -> i32 {
        Self::get_regular_vertex_valence(s)
    }
    #[inline]
    pub fn local_neighborhood_size(s: SchemeType) -> i32 {
        Self::get_local_neighborhood_size(s)
    }
    #[inline]
    pub fn topological_split_type(s: SchemeType) -> Split {
        Self::get_topological_split_type(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_traits() {
        assert_eq!(
            SchemeTypeTraits::get_topological_split_type(SchemeType::Bilinear),
            Split::ToQuads
        );
        assert_eq!(
            SchemeTypeTraits::get_regular_face_size(SchemeType::Bilinear),
            4
        );
        assert_eq!(
            SchemeTypeTraits::get_regular_vertex_valence(SchemeType::Bilinear),
            4
        );
        assert_eq!(
            SchemeTypeTraits::get_local_neighborhood_size(SchemeType::Bilinear),
            0
        );
        assert_eq!(SchemeTypeTraits::get_name(SchemeType::Bilinear), "bilinear");
    }

    #[test]
    fn catmark_traits() {
        assert_eq!(
            SchemeTypeTraits::get_topological_split_type(SchemeType::Catmark),
            Split::ToQuads
        );
        assert_eq!(
            SchemeTypeTraits::get_regular_face_size(SchemeType::Catmark),
            4
        );
        assert_eq!(
            SchemeTypeTraits::get_regular_vertex_valence(SchemeType::Catmark),
            4
        );
        assert_eq!(
            SchemeTypeTraits::get_local_neighborhood_size(SchemeType::Catmark),
            1
        );
        assert_eq!(SchemeTypeTraits::get_name(SchemeType::Catmark), "catmark");
    }

    #[test]
    fn loop_traits() {
        assert_eq!(
            SchemeTypeTraits::get_topological_split_type(SchemeType::Loop),
            Split::ToTris
        );
        assert_eq!(SchemeTypeTraits::get_regular_face_size(SchemeType::Loop), 3);
        assert_eq!(
            SchemeTypeTraits::get_regular_vertex_valence(SchemeType::Loop),
            6
        );
        assert_eq!(
            SchemeTypeTraits::get_local_neighborhood_size(SchemeType::Loop),
            1
        );
        assert_eq!(SchemeTypeTraits::get_name(SchemeType::Loop), "loop");
    }

    #[test]
    fn get_type_identity() {
        assert_eq!(
            SchemeTypeTraits::get_type(SchemeType::Catmark),
            SchemeType::Catmark
        );
        assert_eq!(
            SchemeTypeTraits::get_type(SchemeType::Loop),
            SchemeType::Loop
        );
    }
}
