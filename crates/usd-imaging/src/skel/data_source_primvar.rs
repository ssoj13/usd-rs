//! DataSourcePrimvar - Primvar container for skinning inputs.
//!
//! Port of pxr/usdImaging/usdSkelImaging/dataSourcePrimvar.h/cpp
//!
//! A primvar data source for UsdSkel's skinning primvars.
//! Implements HdContainerDataSource with primvarValue, interpolation, role.

use std::sync::Arc;
use usd_hd::data_source::{
    HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle,
    HdRetainedTypedSampledDataSource,
};
use usd_tf::Token;

/// HdPrimvarSchema token for the primvar value data source.
pub static PRIMVAR_VALUE: std::sync::LazyLock<Token> =
    std::sync::LazyLock::new(|| Token::new("primvarValue"));
/// HdPrimvarSchema token for interpolation.
pub static INTERPOLATION: std::sync::LazyLock<Token> =
    std::sync::LazyLock::new(|| Token::new("interpolation"));
/// HdPrimvarSchema token for role.
pub static ROLE: std::sync::LazyLock<Token> = std::sync::LazyLock::new(|| Token::new("role"));

/// Interpolation constant (one value per prim).
pub static CONSTANT: std::sync::LazyLock<Token> =
    std::sync::LazyLock::new(|| Token::new("constant"));
/// Interpolation uniform (one value per curve/patch).
pub static UNIFORM: std::sync::LazyLock<Token> = std::sync::LazyLock::new(|| Token::new("uniform"));
/// Interpolation varying (one value per vertex).
pub static VARYING: std::sync::LazyLock<Token> = std::sync::LazyLock::new(|| Token::new("varying"));
/// Interpolation vertex (per vertex).
pub static VERTEX: std::sync::LazyLock<Token> = std::sync::LazyLock::new(|| Token::new("vertex"));
/// Interpolation face-varying (per face corner).
pub static FACE_VARYING: std::sync::LazyLock<Token> =
    std::sync::LazyLock::new(|| Token::new("faceVarying"));
/// Interpolation instance (per instance).
pub static INSTANCE: std::sync::LazyLock<Token> =
    std::sync::LazyLock::new(|| Token::new("instance"));

/// Role: point position.
pub static POINT: std::sync::LazyLock<Token> = std::sync::LazyLock::new(|| Token::new("point"));
/// Role: normal vector.
pub static NORMAL: std::sync::LazyLock<Token> = std::sync::LazyLock::new(|| Token::new("normal"));
/// Role: vector.
pub static VECTOR: std::sync::LazyLock<Token> = std::sync::LazyLock::new(|| Token::new("vector"));
/// Role: color.
pub static COLOR: std::sync::LazyLock<Token> = std::sync::LazyLock::new(|| Token::new("color"));
/// Role: point index.
pub static POINT_INDEX: std::sync::LazyLock<Token> =
    std::sync::LazyLock::new(|| Token::new("pointIndex"));
/// Role: edge index.
pub static EDGE_INDEX: std::sync::LazyLock<Token> =
    std::sync::LazyLock::new(|| Token::new("edgeIndex"));
/// Role: face index.
pub static FACE_INDEX: std::sync::LazyLock<Token> =
    std::sync::LazyLock::new(|| Token::new("faceIndex"));
/// Role: texture coordinate.
pub static TEXTURE_COORDINATE: std::sync::LazyLock<Token> =
    std::sync::LazyLock::new(|| Token::new("textureCoordinate"));

/// Static cached data sources (port of HdPrimvarSchema::BuildInterpolationDataSource/BuildRoleDataSource).
/// C++ caches these to avoid allocations for common tokens.
static CACHED_INTERP_CONSTANT: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(CONSTANT.clone()) as HdDataSourceBaseHandle
    });
static CACHED_INTERP_UNIFORM: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(UNIFORM.clone()) as HdDataSourceBaseHandle
    });
static CACHED_INTERP_VARYING: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(VARYING.clone()) as HdDataSourceBaseHandle
    });
static CACHED_INTERP_VERTEX: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(VERTEX.clone()) as HdDataSourceBaseHandle
    });
static CACHED_INTERP_FACE_VARYING: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(FACE_VARYING.clone()) as HdDataSourceBaseHandle
    });
static CACHED_INTERP_INSTANCE: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(INSTANCE.clone()) as HdDataSourceBaseHandle
    });

static CACHED_ROLE_POINT: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(POINT.clone()) as HdDataSourceBaseHandle
    });
static CACHED_ROLE_NORMAL: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(NORMAL.clone()) as HdDataSourceBaseHandle
    });
static CACHED_ROLE_VECTOR: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(VECTOR.clone()) as HdDataSourceBaseHandle
    });
static CACHED_ROLE_COLOR: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(COLOR.clone()) as HdDataSourceBaseHandle
    });
static CACHED_ROLE_POINT_INDEX: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(POINT_INDEX.clone()) as HdDataSourceBaseHandle
    });
static CACHED_ROLE_EDGE_INDEX: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(EDGE_INDEX.clone()) as HdDataSourceBaseHandle
    });
static CACHED_ROLE_FACE_INDEX: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(FACE_INDEX.clone()) as HdDataSourceBaseHandle
    });
static CACHED_ROLE_TEXTURE_COORDINATE: std::sync::LazyLock<HdDataSourceBaseHandle> =
    std::sync::LazyLock::new(|| {
        HdRetainedTypedSampledDataSource::new(TEXTURE_COORDINATE.clone()) as HdDataSourceBaseHandle
    });

/// Build interpolation data source (port of HdPrimvarSchema::BuildInterpolationDataSource).
///
/// Returns static cached instances for constant/uniform/varying/vertex/faceVarying/instance.
fn build_interpolation_data_source(interpolation: &Token) -> HdDataSourceBaseHandle {
    if *interpolation == *CONSTANT {
        return CACHED_INTERP_CONSTANT.clone();
    }
    if *interpolation == *UNIFORM {
        return CACHED_INTERP_UNIFORM.clone();
    }
    if *interpolation == *VARYING {
        return CACHED_INTERP_VARYING.clone();
    }
    if *interpolation == *VERTEX {
        return CACHED_INTERP_VERTEX.clone();
    }
    if *interpolation == *FACE_VARYING {
        return CACHED_INTERP_FACE_VARYING.clone();
    }
    if *interpolation == *INSTANCE {
        return CACHED_INTERP_INSTANCE.clone();
    }
    // Fallback for unknown token
    HdRetainedTypedSampledDataSource::new(interpolation.clone()) as HdDataSourceBaseHandle
}

/// Build role data source (port of HdPrimvarSchema::BuildRoleDataSource).
///
/// Returns static cached instances for point/normal/vector/color/pointIndex/edgeIndex/faceIndex/textureCoordinate.
fn build_role_data_source(role: &Token) -> HdDataSourceBaseHandle {
    if *role == *POINT {
        return CACHED_ROLE_POINT.clone();
    }
    if *role == *NORMAL {
        return CACHED_ROLE_NORMAL.clone();
    }
    if *role == *VECTOR {
        return CACHED_ROLE_VECTOR.clone();
    }
    if *role == *COLOR {
        return CACHED_ROLE_COLOR.clone();
    }
    if *role == *POINT_INDEX {
        return CACHED_ROLE_POINT_INDEX.clone();
    }
    if *role == *EDGE_INDEX {
        return CACHED_ROLE_EDGE_INDEX.clone();
    }
    if *role == *FACE_INDEX {
        return CACHED_ROLE_FACE_INDEX.clone();
    }
    if *role == *TEXTURE_COORDINATE {
        return CACHED_ROLE_TEXTURE_COORDINATE.clone();
    }
    // Fallback for unknown token (including empty TfToken())
    HdRetainedTypedSampledDataSource::new(role.clone()) as HdDataSourceBaseHandle
}

/// A primvar data source for UsdSkel's skinning primvars.
///
/// Port of UsdSkelImaging_DataSourcePrimvar.
/// Implements HdContainerDataSource with primvarValue, interpolation, role.
#[derive(Debug)]
pub struct DataSourcePrimvar {
    value_source: HdDataSourceBaseHandle,
    interpolation: Token,
    role: Token,
}

impl DataSourcePrimvar {
    /// Create new primvar data source (port of UsdSkelImaging_DataSourcePrimvar::New).
    ///
    /// * `value_source` - The primvar value (e.g. skinningXforms, blendShapeWeights)
    /// * `interpolation` - Default "constant" (HdPrimvarSchemaTokens->constant)
    /// * `role` - Default empty (TfToken())
    pub fn new(
        value_source: HdDataSourceBaseHandle,
        interpolation: Token,
        role: Token,
    ) -> Arc<Self> {
        Arc::new(Self {
            value_source,
            interpolation,
            role,
        })
    }

    /// Create with default interpolation (constant) and empty role.
    pub fn new_default(value_source: HdDataSourceBaseHandle) -> Arc<Self> {
        Self::new(value_source, CONSTANT.clone(), Token::new(""))
    }
}

impl HdDataSourceBase for DataSourcePrimvar {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            value_source: self.value_source.clone(),
            interpolation: self.interpolation.clone(),
            role: self.role.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            value_source: self.value_source.clone(),
            interpolation: self.interpolation.clone(),
            role: self.role.clone(),
        }))
    }
}

impl HdContainerDataSource for DataSourcePrimvar {
    fn get_names(&self) -> Vec<Token> {
        vec![PRIMVAR_VALUE.clone(), INTERPOLATION.clone(), ROLE.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *PRIMVAR_VALUE {
            return Some(self.value_source.clone());
        }
        if *name == *INTERPOLATION {
            return Some(build_interpolation_data_source(&self.interpolation));
        }
        if *name == *ROLE {
            return Some(build_role_data_source(&self.role));
        }
        None
    }
}

/// Handle type for DataSourcePrimvar (port of HD_DECLARE_DATASOURCE_HANDLES).
pub type DataSourcePrimvarHandle = Arc<DataSourcePrimvar>;

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::data_source::cast_to_container;

    #[test]
    fn test_data_source_primvar_advertises_container_interface() {
        let value_source =
            HdRetainedTypedSampledDataSource::new(1i32) as HdDataSourceBaseHandle;
        let primvar = DataSourcePrimvar::new_default(value_source) as HdDataSourceBaseHandle;
        assert!(cast_to_container(&primvar).is_some());
    }
}
