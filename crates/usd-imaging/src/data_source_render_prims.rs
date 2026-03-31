//! Render prim data sources matching `dataSourceRenderPrims.cpp`.

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use crate::usd_render_product_schema::UsdRenderProductSchema;
use crate::usd_render_settings_schema::UsdRenderSettingsSchema;
use crate::usd_render_var_schema::UsdRenderVarSchema;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::schema::HdRenderPassSchema;
use usd_hd::utils::convert_vt_dictionary_to_container_ds;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocatorSet, HdRetainedTypedSampledDataSource,
};
use usd_render::{
    RenderPass, RenderProduct, RenderSettings, RenderSettingsBase, RenderVar,
    compute_namespaced_settings,
};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;
use usd_vt::Value;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static NAMESPACED_SETTINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("namespacedSettings"));
    pub static CAMERA: LazyLock<Token> = LazyLock::new(|| Token::new("camera"));
    pub static PRODUCTS: LazyLock<Token> = LazyLock::new(|| Token::new("products"));
    pub static ORDERED_VARS: LazyLock<Token> = LazyLock::new(|| Token::new("orderedVars"));
    pub static PASS_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("passType"));
    pub static RENDER_SOURCE: LazyLock<Token> = LazyLock::new(|| Token::new("renderSource"));
    pub static RI_INTEGRATOR: LazyLock<Token> = LazyLock::new(|| Token::new("ri:integrator"));
    pub static RI_SAMPLE_FILTERS: LazyLock<Token> =
        LazyLock::new(|| Token::new("ri:sampleFilters"));
    pub static RI_DISPLAY_FILTERS: LazyLock<Token> =
        LazyLock::new(|| Token::new("ri:displayFilters"));
}

fn concat(a: Vec<Token>, b: &[Token]) -> Vec<Token> {
    let mut result = Vec::with_capacity(a.len() + b.len());
    result.extend(a);
    result.extend(b.iter().cloned());
    result
}

fn strip_relationships_from_settings(prim: &Prim, settings: &mut HashMap<String, Value>) {
    let mut to_erase = Vec::new();
    for name in settings.keys() {
        let token = Token::new(name);
        if prim.get_relationship(name).is_some()
            && token != *tokens::RI_INTEGRATOR
            && token != *tokens::RI_SAMPLE_FILTERS
            && token != *tokens::RI_DISPLAY_FILTERS
        {
            to_erase.push(name.clone());
        }
    }
    for name in to_erase {
        settings.remove(&name);
    }
}

fn compute_namespaced_settings_ds(prim: &Prim) -> HdContainerDataSourceHandle {
    let mut settings = compute_namespaced_settings(prim, &[]);
    strip_relationships_from_settings(prim, &mut settings);
    convert_vt_dictionary_to_container_ds(&settings)
}

fn first_forwarded_target(rel: Option<usd_core::Relationship>) -> Option<Path> {
    let rel = rel?;
    rel.get_forwarded_targets().into_iter().next()
}

fn forwarded_targets(rel: Option<usd_core::Relationship>) -> Vec<Path> {
    rel.map(|r| r.get_forwarded_targets()).unwrap_or_default()
}

fn render_pass_property_names() -> Vec<Token> {
    vec![tokens::PASS_TYPE.clone(), tokens::RENDER_SOURCE.clone()]
}

fn render_settings_property_names() -> Vec<Token> {
    concat(
        RenderSettings::get_schema_attribute_names(true),
        &[
            tokens::NAMESPACED_SETTINGS.clone(),
            tokens::CAMERA.clone(),
            tokens::PRODUCTS.clone(),
        ],
    )
}

fn render_product_property_names() -> Vec<Token> {
    concat(
        RenderProduct::get_schema_attribute_names(true),
        &[
            tokens::NAMESPACED_SETTINGS.clone(),
            tokens::CAMERA.clone(),
            tokens::ORDERED_VARS.clone(),
        ],
    )
}

fn render_var_property_names() -> Vec<Token> {
    concat(
        RenderVar::get_schema_attribute_names(true),
        &[tokens::NAMESPACED_SETTINGS.clone()],
    )
}

#[derive(Clone)]
struct DataSourceRenderPass {
    usd_render_pass: RenderPass,
}

impl DataSourceRenderPass {
    fn property_names() -> Vec<Token> {
        render_pass_property_names()
    }
}

impl std::fmt::Debug for DataSourceRenderPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceRenderPass")
    }
}

impl HdDataSourceBase for DataSourceRenderPass {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceRenderPass {
    fn get_names(&self) -> Vec<Token> {
        Self::property_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::PASS_TYPE {
            let pass_type = self
                .usd_render_pass
                .get_pass_type_attr()
                .and_then(|attr| attr.get(TimeCode::default()))
                .and_then(|value| value.get::<Token>().cloned())?;
            return Some(HdRetainedTypedSampledDataSource::new(pass_type));
        }
        if name == &*tokens::RENDER_SOURCE {
            let render_source = first_forwarded_target(self.usd_render_pass.get_render_source_rel())?;
            return Some(HdRetainedTypedSampledDataSource::new(render_source));
        }
        None
    }
}

#[derive(Clone)]
struct DataSourceRenderSettings {
    scene_index_path: Path,
    usd_render_settings: RenderSettings,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceRenderSettings {
    fn property_names() -> Vec<Token> {
        render_settings_property_names()
    }
}

impl std::fmt::Debug for DataSourceRenderSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceRenderSettings")
    }
}

impl HdDataSourceBase for DataSourceRenderSettings {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceRenderSettings {
    fn get_names(&self) -> Vec<Token> {
        Self::property_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::NAMESPACED_SETTINGS {
            return Some(compute_namespaced_settings_ds(self.usd_render_settings.get_prim()));
        }
        if name == &*tokens::CAMERA {
            let target = first_forwarded_target(
                self.usd_render_settings.as_settings_base().get_camera_rel(),
            )?;
            return Some(HdRetainedTypedSampledDataSource::new(target));
        }
        if name == &*tokens::PRODUCTS {
            return Some(HdRetainedTypedSampledDataSource::new(forwarded_targets(
                self.usd_render_settings.get_products_rel(),
            )));
        }

        let attr = self.usd_render_settings.get_prim().get_attribute(name.as_str())?;
        Some(DataSourceAttribute::<Value>::new(
            attr,
            self.stage_globals.clone(),
            self.scene_index_path.clone(),
        ))
    }
}

#[derive(Clone)]
struct DataSourceRenderProduct {
    scene_index_path: Path,
    usd_render_product: RenderProduct,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceRenderProduct {
    fn property_names() -> Vec<Token> {
        render_product_property_names()
    }
}

impl std::fmt::Debug for DataSourceRenderProduct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceRenderProduct")
    }
}

impl HdDataSourceBase for DataSourceRenderProduct {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceRenderProduct {
    fn get_names(&self) -> Vec<Token> {
        Self::property_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::NAMESPACED_SETTINGS {
            return Some(compute_namespaced_settings_ds(self.usd_render_product.get_prim()));
        }
        if name == &*tokens::CAMERA {
            let target = first_forwarded_target(self.usd_render_product.as_settings_base().get_camera_rel())?;
            return Some(HdRetainedTypedSampledDataSource::new(target));
        }
        if name == &*tokens::ORDERED_VARS {
            return Some(HdRetainedTypedSampledDataSource::new(forwarded_targets(
                self.usd_render_product.get_ordered_vars_rel(),
            )));
        }

        let attr = self.usd_render_product.get_prim().get_attribute(name.as_str())?;
        let settings_base_names: HashSet<Token> = RenderSettingsBase::get_schema_attribute_names(true)
            .into_iter()
            .collect();
        if settings_base_names.contains(name) && !attr.has_authored_value() {
            return None;
        }

        Some(DataSourceAttribute::<Value>::new(
            attr,
            self.stage_globals.clone(),
            self.scene_index_path.clone(),
        ))
    }
}

#[derive(Clone)]
struct DataSourceRenderVar {
    scene_index_path: Path,
    usd_render_var: RenderVar,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceRenderVar {
    fn property_names() -> Vec<Token> {
        render_var_property_names()
    }
}

impl std::fmt::Debug for DataSourceRenderVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceRenderVar")
    }
}

impl HdDataSourceBase for DataSourceRenderVar {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceRenderVar {
    fn get_names(&self) -> Vec<Token> {
        Self::property_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::NAMESPACED_SETTINGS {
            return Some(compute_namespaced_settings_ds(self.usd_render_var.get_prim()));
        }
        let attr = self.usd_render_var.get_prim().get_attribute(name.as_str())?;
        Some(DataSourceAttribute::<Value>::new(
            attr,
            self.stage_globals.clone(),
            self.scene_index_path.clone(),
        ))
    }
}

#[derive(Clone)]
pub struct DataSourceRenderPassPrim {
    scene_index_path: Path,
    prim: Prim,
}

impl DataSourceRenderPassPrim {
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        _stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
        }
    }

    pub fn get_names(&self) -> Vec<Token> {
        vec![(*HdRenderPassSchema::get_schema_token()).clone()]
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*HdRenderPassSchema::get_schema_token() {
            return Some(Arc::new(DataSourceRenderPass {
                usd_render_pass: RenderPass::new(self.prim.clone()),
            }));
        }
        None
    }

    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let known: HashSet<Token> = DataSourceRenderPass::property_names().into_iter().collect();
        let mut locators = HdDataSourceLocatorSet::empty();
        for property in properties {
            if known.contains(property) {
                locators.insert(HdRenderPassSchema::get_default_locator().append(property));
            }
        }
        locators
    }
}

impl std::fmt::Debug for DataSourceRenderPassPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceRenderPassPrim")
            .field("scene_index_path", &self.scene_index_path)
            .finish()
    }
}

impl HdDataSourceBase for DataSourceRenderPassPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceRenderPassPrim {
    fn get_names(&self) -> Vec<Token> {
        DataSourceRenderPassPrim::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        DataSourceRenderPassPrim::get(self, name)
    }
}

pub type DataSourceRenderPassPrimHandle = Arc<DataSourceRenderPassPrim>;

#[derive(Clone)]
pub struct DataSourceRenderSettingsPrim {
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceRenderSettingsPrim {
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
            stage_globals,
        }
    }

    pub fn get_names(&self) -> Vec<Token> {
        vec![UsdRenderSettingsSchema::get_schema_token()]
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &UsdRenderSettingsSchema::get_schema_token() {
            return Some(Arc::new(DataSourceRenderSettings {
                scene_index_path: self.scene_index_path.clone(),
                usd_render_settings: RenderSettings::new(self.prim.clone()),
                stage_globals: self.stage_globals.clone(),
            }));
        }
        None
    }

    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        invalidate_known_or_namespaced(
            properties,
            &render_settings_property_names(),
            UsdRenderSettingsSchema::get_default_locator(),
            UsdRenderSettingsSchema::get_namespaced_settings_locator(),
        )
    }
}

impl std::fmt::Debug for DataSourceRenderSettingsPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceRenderSettingsPrim")
            .field("scene_index_path", &self.scene_index_path)
            .finish()
    }
}

impl HdDataSourceBase for DataSourceRenderSettingsPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceRenderSettingsPrim {
    fn get_names(&self) -> Vec<Token> {
        DataSourceRenderSettingsPrim::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        DataSourceRenderSettingsPrim::get(self, name)
    }
}

pub type DataSourceRenderSettingsPrimHandle = Arc<DataSourceRenderSettingsPrim>;

#[derive(Clone)]
pub struct DataSourceRenderProductPrim {
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceRenderProductPrim {
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
            stage_globals,
        }
    }

    pub fn get_names(&self) -> Vec<Token> {
        vec![UsdRenderProductSchema::get_schema_token()]
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &UsdRenderProductSchema::get_schema_token() {
            return Some(Arc::new(DataSourceRenderProduct {
                scene_index_path: self.scene_index_path.clone(),
                usd_render_product: RenderProduct::new(self.prim.clone()),
                stage_globals: self.stage_globals.clone(),
            }));
        }
        None
    }

    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        invalidate_known_or_namespaced(
            properties,
            &render_product_property_names(),
            UsdRenderProductSchema::get_default_locator(),
            UsdRenderProductSchema::get_namespaced_settings_locator(),
        )
    }
}

impl std::fmt::Debug for DataSourceRenderProductPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceRenderProductPrim")
            .field("scene_index_path", &self.scene_index_path)
            .finish()
    }
}

impl HdDataSourceBase for DataSourceRenderProductPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceRenderProductPrim {
    fn get_names(&self) -> Vec<Token> {
        DataSourceRenderProductPrim::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        DataSourceRenderProductPrim::get(self, name)
    }
}

pub type DataSourceRenderProductPrimHandle = Arc<DataSourceRenderProductPrim>;

#[derive(Clone)]
pub struct DataSourceRenderVarPrim {
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceRenderVarPrim {
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
            stage_globals,
        }
    }

    pub fn get_names(&self) -> Vec<Token> {
        vec![UsdRenderVarSchema::get_schema_token()]
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &UsdRenderVarSchema::get_schema_token() {
            return Some(Arc::new(DataSourceRenderVar {
                scene_index_path: self.scene_index_path.clone(),
                usd_render_var: RenderVar::new(self.prim.clone()),
                stage_globals: self.stage_globals.clone(),
            }));
        }
        None
    }

    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        invalidate_known_or_namespaced(
            properties,
            &render_var_property_names(),
            UsdRenderVarSchema::get_default_locator(),
            UsdRenderVarSchema::get_namespaced_settings_locator(),
        )
    }
}

impl std::fmt::Debug for DataSourceRenderVarPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceRenderVarPrim")
            .field("scene_index_path", &self.scene_index_path)
            .finish()
    }
}

impl HdDataSourceBase for DataSourceRenderVarPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceRenderVarPrim {
    fn get_names(&self) -> Vec<Token> {
        DataSourceRenderVarPrim::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        DataSourceRenderVarPrim::get(self, name)
    }
}

pub type DataSourceRenderVarPrimHandle = Arc<DataSourceRenderVarPrim>;

fn invalidate_known_or_namespaced(
    properties: &[Token],
    names: &[Token],
    default_locator: usd_hd::HdDataSourceLocator,
    namespaced_settings_locator: usd_hd::HdDataSourceLocator,
) -> HdDataSourceLocatorSet {
    let known: HashSet<Token> = names.iter().cloned().collect();
    let mut locators = HdDataSourceLocatorSet::empty();
    for property in properties {
        if known.contains(property) {
            locators.insert(default_locator.append(property));
        } else {
            locators.insert(namespaced_settings_locator.clone());
        }
    }
    locators
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;
    use usd_core::common::InitialLoadSet;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_render_settings_prim_names_match_reference() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        let ds = DataSourceRenderSettingsPrim::new(Path::absolute_root(), prim, create_test_globals());
        let names = ds.get_names();
        assert_eq!(names, vec![UsdRenderSettingsSchema::get_schema_token()]);
    }

    #[test]
    fn test_render_product_unknown_property_dirties_namespaced_settings() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        let locators = DataSourceRenderProductPrim::invalidate(
            &prim,
            &Token::new(""),
            &[Token::new("ri:customSetting")],
            PropertyInvalidationType::PropertyChanged,
        );
        assert!(locators.contains(&UsdRenderProductSchema::get_namespaced_settings_locator()));
    }

    #[test]
    fn test_render_pass_prim_names_match_reference() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        let ds = DataSourceRenderPassPrim::new(Path::absolute_root(), prim, create_test_globals());
        let names = ds.get_names();
        assert_eq!(names, vec![(*HdRenderPassSchema::get_schema_token()).clone()]);
    }
}
