//! DataSourceUsdPrimInfo - USD prim metadata data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceUsdPrimInfo.h
//!
//! A container data source containing metadata such as the specifier
//! of a prim or native instancing information.

use std::sync::Arc;
use usd_core::Prim;
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::{HdContainerDataSource, HdDataSourceBaseHandle};
use usd_sdf::Specifier;
use usd_tf::Token;

// Token constants
#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static USD_PRIM_INFO: LazyLock<Token> = LazyLock::new(|| Token::new("__usdPrimInfo"));
    pub static SPECIFIER: LazyLock<Token> = LazyLock::new(|| Token::new("specifier"));
    pub static TYPE_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("typeName"));
    pub static IS_LOADED: LazyLock<Token> = LazyLock::new(|| Token::new("isLoaded"));
    pub static API_SCHEMAS: LazyLock<Token> = LazyLock::new(|| Token::new("apiSchemas"));
    pub static KIND: LazyLock<Token> = LazyLock::new(|| Token::new("kind"));
    pub static NI_PROTOTYPE_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("niPrototypePath"));
    pub static IS_NI_PROTOTYPE: LazyLock<Token> = LazyLock::new(|| Token::new("isNiPrototype"));

    // Specifier token values
    pub static DEF: LazyLock<Token> = LazyLock::new(|| Token::new("def"));
    pub static OVER: LazyLock<Token> = LazyLock::new(|| Token::new("over"));
    pub static CLASS: LazyLock<Token> = LazyLock::new(|| Token::new("class"));
}

// ============================================================================
// DataSourceUsdPrimInfo
// ============================================================================

/// Container data source for USD prim metadata.
///
/// Contains the specifier, loaded state, and native instancing info.
#[derive(Clone)]
pub struct DataSourceUsdPrimInfo {
    /// The USD prim
    prim: Prim,
}

impl DataSourceUsdPrimInfo {
    /// Create a new prim info data source.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Get the schema token for this data source.
    pub fn get_schema_token() -> Token {
        tokens::USD_PRIM_INFO.clone()
    }
}

impl std::fmt::Debug for DataSourceUsdPrimInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceUsdPrimInfo")
    }
}

impl usd_hd::HdDataSourceBase for DataSourceUsdPrimInfo {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        std::sync::Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

/// Convert specifier enum to a cached token data source.
fn specifier_to_ds(spec: Specifier) -> HdDataSourceBaseHandle {
    match spec {
        Specifier::Def => {
            HdRetainedTypedSampledDataSource::new(tokens::DEF.clone()) as HdDataSourceBaseHandle
        }
        Specifier::Over => {
            HdRetainedTypedSampledDataSource::new(tokens::OVER.clone()) as HdDataSourceBaseHandle
        }
        Specifier::Class => {
            HdRetainedTypedSampledDataSource::new(tokens::CLASS.clone()) as HdDataSourceBaseHandle
        }
    }
}

impl HdContainerDataSource for DataSourceUsdPrimInfo {
    fn get_names(&self) -> Vec<Token> {
        let mut names = vec![
            tokens::SPECIFIER.clone(),
            tokens::TYPE_NAME.clone(),
            tokens::IS_LOADED.clone(),
            tokens::API_SCHEMAS.clone(),
            tokens::KIND.clone(),
        ];

        if self.prim.is_instance() {
            names.push(tokens::NI_PROTOTYPE_PATH.clone());
        }
        if self.prim.is_prototype() {
            names.push(tokens::IS_NI_PROTOTYPE.clone());
        }

        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::SPECIFIER {
            return Some(specifier_to_ds(self.prim.specifier()));
        }
        if name == &*tokens::TYPE_NAME {
            return Some(HdRetainedTypedSampledDataSource::new(self.prim.type_name())
                as HdDataSourceBaseHandle);
        }
        if name == &*tokens::IS_LOADED {
            return Some(HdRetainedTypedSampledDataSource::new(self.prim.is_loaded())
                as HdDataSourceBaseHandle);
        }
        if name == &*tokens::API_SCHEMAS {
            let schemas = self.prim.get_applied_schemas();
            if schemas.is_empty() {
                return None;
            }
            return Some(HdRetainedTypedSampledDataSource::new(schemas) as HdDataSourceBaseHandle);
        }
        if name == &*tokens::KIND {
            let kind = self.prim.get_kind()?;
            return Some(HdRetainedTypedSampledDataSource::new(kind) as HdDataSourceBaseHandle);
        }
        if name == &*tokens::NI_PROTOTYPE_PATH {
            if !self.prim.is_instance() {
                return None;
            }
            let prototype = self.prim.get_prototype();
            if !prototype.is_valid() {
                return None;
            }
            return Some(
                HdRetainedTypedSampledDataSource::new(prototype.get_path().clone())
                    as HdDataSourceBaseHandle,
            );
        }
        if name == &*tokens::IS_NI_PROTOTYPE {
            if !self.prim.is_prototype() {
                return None;
            }
            return Some(HdRetainedTypedSampledDataSource::new(true) as HdDataSourceBaseHandle);
        }
        None
    }
}

/// Handle type for DataSourceUsdPrimInfo.
pub type DataSourceUsdPrimInfoHandle = Arc<DataSourceUsdPrimInfo>;

/// Factory function for creating prim info data sources.
pub fn create_data_source_usd_prim_info(prim: Prim) -> DataSourceUsdPrimInfoHandle {
    Arc::new(DataSourceUsdPrimInfo::new(prim))
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            DataSourceUsdPrimInfo::get_schema_token().as_str(),
            "__usdPrimInfo"
        );
    }

    #[test]
    fn test_prim_info_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let ds = DataSourceUsdPrimInfo::new(prim);

        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "specifier"));
        assert!(names.iter().any(|n| n == "isLoaded"));
    }
}
