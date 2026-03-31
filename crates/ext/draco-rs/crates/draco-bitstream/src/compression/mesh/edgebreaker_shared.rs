//! Shared edgebreaker declarations.
//! Reference: `_ref/draco/src/draco/compression/mesh/mesh_edgebreaker_shared.h`.

/// Bit patterns used to encode edgebreaker topology symbols.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgebreakerTopologyBitPattern {
    TopologyC = 0x0,
    TopologyS = 0x1,
    TopologyL = 0x3,
    TopologyR = 0x5,
    TopologyE = 0x7,
    /// Special symbol for initial face, not encoded.
    TopologyInitFace = 0x8,
    /// Invalid symbol marker.
    TopologyInvalid = 0x9,
}

/// Symbol ids corresponding to topology patterns.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgebreakerSymbol {
    SymbolC = 0,
    SymbolS = 1,
    SymbolL = 2,
    SymbolR = 3,
    SymbolE = 4,
    SymbolInvalid = 5,
}

/// Edge sides relative to the tip vertex of a visited triangle.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeFaceName {
    LeftFaceEdge = 0,
    RightFaceEdge = 1,
}

/// Bit-length of symbols in the EdgebreakerTopologyBitPattern.
pub const EDGE_BREAKER_TOPOLOGY_BIT_PATTERN_LENGTH: [i32; 8] = [1, 3, 0, 3, 0, 3, 0, 3];

/// Zero-indexed symbol id for each topology pattern.
pub const EDGE_BREAKER_TOPOLOGY_TO_SYMBOL_ID: [EdgebreakerSymbol; 8] = [
    EdgebreakerSymbol::SymbolC,
    EdgebreakerSymbol::SymbolS,
    EdgebreakerSymbol::SymbolInvalid,
    EdgebreakerSymbol::SymbolL,
    EdgebreakerSymbol::SymbolInvalid,
    EdgebreakerSymbol::SymbolR,
    EdgebreakerSymbol::SymbolInvalid,
    EdgebreakerSymbol::SymbolE,
];

/// Reverse mapping between symbol id and topology pattern symbol.
pub const EDGE_BREAKER_SYMBOL_TO_TOPOLOGY_ID: [EdgebreakerTopologyBitPattern; 5] = [
    EdgebreakerTopologyBitPattern::TopologyC,
    EdgebreakerTopologyBitPattern::TopologyS,
    EdgebreakerTopologyBitPattern::TopologyL,
    EdgebreakerTopologyBitPattern::TopologyR,
    EdgebreakerTopologyBitPattern::TopologyE,
];

/// Data for topology split events.
#[derive(Clone, Copy, Debug, Default)]
pub struct TopologySplitEventData {
    pub split_symbol_id: u32,
    pub source_symbol_id: u32,
    pub source_edge: u32,
}

/// Data for hole events.
#[derive(Clone, Copy, Debug, Default)]
pub struct HoleEventData {
    pub symbol_id: i32,
}

impl HoleEventData {
    pub fn new(symbol_id: i32) -> Self {
        Self { symbol_id }
    }
}

/// Supported modes for valence-based edgebreaker coding.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgebreakerValenceCodingMode {
    EdgebreakerValenceMode2To7 = 0,
}
