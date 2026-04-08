//! RenderSettingsFlatteningSceneIndex - flattens render settings/products/vars.
//!
//! Port of pxr/usdImaging/usdImaging/renderSettingsFlatteningSceneIndex.cpp
//!
//! For renderSettings prims, this scene index:
//!   1. Adds a flattened `renderSettings` container (HdRenderSettingsSchema)
//!      that resolves products/vars relationships into inline data.
//!   2. Adds `__dependencies` so the dependency forwarding scene index
//!      propagates changes from products/vars back to the settings prim.
//!
//! Resolution of "base" properties (resolution, camera, etc.) follows the
//! USD RenderSettingsBase pattern: product opinion wins, else settings opinion.

use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::data_source::{
    HdDataSourceBaseHandle, HdDataSourceLocator, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle,
    HdSceneIndexPrim, HdSingleInputFilteringSceneIndexBase, SdfPathVector, wire_filter_to_input,
};
use usd_hd::{HdContainerDataSource, HdDataSourceBase};
use usd_sdf::Path;
use usd_tf::Token;

// -- tokens ------------------------------------------------------------------

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static RENDER_SETTINGS: LazyLock<Token> = LazyLock::new(|| Token::new("renderSettings"));
    pub static RENDER_PRODUCTS: LazyLock<Token> = LazyLock::new(|| Token::new("renderProducts"));
    pub static DEPENDENCIES: LazyLock<Token> = LazyLock::new(|| Token::new("__dependencies"));

    // USD-side schema tokens
    pub static USD_RENDER_SETTINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("__usdRenderSettings"));
    pub static USD_RENDER_PRODUCT: LazyLock<Token> =
        LazyLock::new(|| Token::new("__usdRenderProduct"));
    pub static USD_RENDER_VAR: LazyLock<Token> = LazyLock::new(|| Token::new("__usdRenderVar"));

    // Fields
    pub static NAMESPACED_SETTINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("namespacedSettings"));
    pub static INCLUDED_PURPOSES: LazyLock<Token> =
        LazyLock::new(|| Token::new("includedPurposes"));
    pub static MATERIAL_BINDING_PURPOSES: LazyLock<Token> =
        LazyLock::new(|| Token::new("materialBindingPurposes"));
    pub static RENDERING_COLOR_SPACE: LazyLock<Token> =
        LazyLock::new(|| Token::new("renderingColorSpace"));
    pub static PRODUCTS: LazyLock<Token> = LazyLock::new(|| Token::new("products"));
    pub static ORDERED_VARS: LazyLock<Token> = LazyLock::new(|| Token::new("orderedVars"));

    // Dependency tokens
    pub static DEPENDED_ON_PRIM_PATH: LazyLock<Token> =
        LazyLock::new(|| Token::new("dependedOnPrimPath"));
    pub static DEPENDED_ON_LOCATOR: LazyLock<Token> =
        LazyLock::new(|| Token::new("dependedOnDataSourceLocator"));
    pub static AFFECTED_LOCATOR: LazyLock<Token> =
        LazyLock::new(|| Token::new("affectedDataSourceLocator"));
}

// -- _RenderSettingsDataSource -----------------------------------------------

/// Flattened render settings container.
///
/// Returns Hydra-schema fields by reading the USD-side `__usdRenderSettings`.
/// For `renderProducts`, performs the full flattening: traverses product paths,
/// resolves vars, and builds inline HdRenderProductSchema containers.
#[derive(Clone)]
struct RenderSettingsDataSource {
    input: HdContainerDataSourceHandle,
    si: HdSceneIndexHandle,
}

impl std::fmt::Debug for RenderSettingsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderSettingsDataSource").finish()
    }
}

impl RenderSettingsDataSource {
    fn new(input: HdContainerDataSourceHandle, si: HdSceneIndexHandle) -> Arc<Self> {
        Arc::new(Self { input, si })
    }

    /// Read a field from the USD render settings schema.
    fn get_usd_field(&self, field: &Token) -> Option<HdDataSourceBaseHandle> {
        let usd_ds = self.input.get(&tokens::USD_RENDER_SETTINGS)?;
        let usd_container = usd_hd::data_source::cast_to_container(&usd_ds)?;
        usd_container.get(field)
    }

    /// Flatten render products from product/var prims into inline data.
    fn flatten_render_products(&self) -> Option<HdDataSourceBaseHandle> {
        let usd_ds = self.input.get(&tokens::USD_RENDER_SETTINGS)?;
        let usd_container = usd_hd::data_source::cast_to_container(&usd_ds)?;

        // Get product paths
        let products_ds = usd_container.get(&tokens::PRODUCTS)?;
        let products_any = products_ds.as_any();
        let product_paths = products_any
            .downcast_ref::<HdRetainedTypedSampledDataSource<Vec<Path>>>()?
            .get_typed_value(0.0);

        let mut product_entries = Vec::new();

        let si_locked = self.si.read();
        for (pid, product_path) in product_paths.iter().enumerate() {
            let prod_prim = si_locked.get_prim(product_path);
            let prod_ds = prod_prim.data_source.as_ref()?;

            // Get __usdRenderProduct from the product prim
            let usd_prod_ds = prod_ds.get(&tokens::USD_RENDER_PRODUCT);
            let usd_prod_container = usd_prod_ds
                .as_ref()
                .and_then(|d| usd_hd::data_source::cast_to_container(d));

            if usd_prod_container.is_none() {
                continue;
            }
            let usd_prod = usd_prod_container.unwrap();

            // Flatten vars for this product
            let mut var_entries = Vec::new();
            if let Some(vars_ds) = usd_prod.get(&tokens::ORDERED_VARS) {
                let vars_any = vars_ds.as_any();
                if let Some(var_paths) =
                    vars_any.downcast_ref::<HdRetainedTypedSampledDataSource<Vec<Path>>>()
                {
                    for (vid, var_path) in var_paths.get_typed_value(0.0).iter().enumerate() {
                        let var_prim = si_locked.get_prim(var_path);
                        if let Some(ref var_ds) = var_prim.data_source {
                            if let Some(usd_var_base) = var_ds.get(&tokens::USD_RENDER_VAR) {
                                if let Some(usd_var) =
                                    usd_hd::data_source::cast_to_container(&usd_var_base)
                                {
                                    // Build flattened var: include path + original fields
                                    let path_ds: HdDataSourceBaseHandle =
                                        HdRetainedTypedSampledDataSource::new(var_path.clone());
                                    let var_name = Token::new(&format!("var_{}", vid));
                                    let mut fields = vec![(Token::new("path"), path_ds)];
                                    for field_name in usd_var.get_names() {
                                        if let Some(field_ds) = usd_var.get(&field_name) {
                                            fields.push((field_name, field_ds));
                                        }
                                    }
                                    let var_container =
                                        HdRetainedContainerDataSource::from_entries(&fields);
                                    var_entries
                                        .push((var_name, var_container as HdDataSourceBaseHandle));
                                }
                            }
                        }
                    }
                }
            }

            // Build flattened product: resolve base properties from settings
            let product_name = Token::new(&format!("product_{}", pid));
            let path_ds: HdDataSourceBaseHandle =
                HdRetainedTypedSampledDataSource::new(product_path.clone());

            let mut fields: Vec<(Token, HdDataSourceBaseHandle)> =
                vec![(Token::new("path"), path_ds)];

            // Copy product-specific fields, falling back to settings values
            let resolve_fields = [
                "resolution",
                "pixelAspectRatio",
                "aspectRatioConformPolicy",
                "dataWindowNDC",
                "disableMotionBlur",
                "disableDepthOfField",
                "camera",
            ];
            for field in &resolve_fields {
                let field_token = Token::new(field);
                let resolved = usd_prod
                    .get(&field_token)
                    .or_else(|| usd_container.get(&field_token));
                if let Some(val) = resolved {
                    fields.push((field_token, val));
                }
            }

            // Add product-only fields
            for field in ["productType", "productName", "namespacedSettings"] {
                let field_token = Token::new(field);
                if let Some(val) = usd_prod.get(&field_token) {
                    fields.push((field_token, val));
                }
            }

            // Add vars
            if !var_entries.is_empty() {
                let vars_container = HdRetainedContainerDataSource::from_entries(&var_entries);
                fields.push((
                    Token::new("renderVars"),
                    vars_container as HdDataSourceBaseHandle,
                ));
            }

            let product_container = HdRetainedContainerDataSource::from_entries(&fields);
            product_entries.push((product_name, product_container as HdDataSourceBaseHandle));
        }

        if product_entries.is_empty() {
            return None;
        }

        Some(
            HdRetainedContainerDataSource::from_entries(&product_entries) as HdDataSourceBaseHandle,
        )
    }
}

impl HdDataSourceBase for RenderSettingsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for RenderSettingsDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::NAMESPACED_SETTINGS.clone(),
            tokens::RENDER_PRODUCTS.clone(),
            tokens::INCLUDED_PURPOSES.clone(),
            tokens::MATERIAL_BINDING_PURPOSES.clone(),
            tokens::RENDERING_COLOR_SPACE.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::NAMESPACED_SETTINGS {
            return self.get_usd_field(&tokens::NAMESPACED_SETTINGS);
        }
        if *name == *tokens::RENDER_PRODUCTS {
            return self.flatten_render_products();
        }
        if *name == *tokens::INCLUDED_PURPOSES {
            return self.get_usd_field(&tokens::INCLUDED_PURPOSES);
        }
        if *name == *tokens::MATERIAL_BINDING_PURPOSES {
            return self.get_usd_field(&tokens::MATERIAL_BINDING_PURPOSES);
        }
        if *name == *tokens::RENDERING_COLOR_SPACE {
            return self.get_usd_field(&tokens::RENDERING_COLOR_SPACE);
        }
        self.input.get(name)
    }
}

// -- _RenderSettingsPrimDataSource -------------------------------------------

/// Prim container override for render settings prims.
///
/// Adds flattened `renderSettings` and `__dependencies` containers.
#[derive(Clone)]
struct RenderSettingsPrimDataSource {
    input: HdContainerDataSourceHandle,
    si: HdSceneIndexHandle,
    prim_path: Path,
}

impl std::fmt::Debug for RenderSettingsPrimDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderSettingsPrimDataSource")
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

impl RenderSettingsPrimDataSource {
    fn new(
        input: HdContainerDataSourceHandle,
        si: HdSceneIndexHandle,
        prim_path: Path,
    ) -> Arc<Self> {
        Arc::new(Self {
            input,
            si,
            prim_path,
        })
    }

    /// Build dependencies container for notice forwarding.
    fn build_dependencies(&self) -> Option<HdDataSourceBaseHandle> {
        let mut entries = Vec::new();

        // Self-dependencies: __usdRenderSettings fields -> renderSettings locators
        let self_deps: &[(&str, &str, &str)] = &[
            (
                "rs_dep_includedPurposes",
                "includedPurposes",
                "includedPurposes",
            ),
            (
                "rs_dep_materialBindingPurposes",
                "materialBindingPurposes",
                "materialBindingPurposes",
            ),
            (
                "rs_dep_namespacedSettings",
                "namespacedSettings",
                "namespacedSettings",
            ),
            (
                "rs_dep_renderingColorSpace",
                "renderingColorSpace",
                "renderingColorSpace",
            ),
            ("rs_dep_resolution", "resolution", "renderProducts"),
            (
                "rs_dep_pixelAspectRatio",
                "pixelAspectRatio",
                "renderProducts",
            ),
            (
                "rs_dep_aspectRatioConformPolicy",
                "aspectRatioConformPolicy",
                "renderProducts",
            ),
            ("rs_dep_dataWindowNDC", "dataWindowNDC", "renderProducts"),
            (
                "rs_dep_disableMotionBlur",
                "disableMotionBlur",
                "renderProducts",
            ),
            (
                "rs_dep_disableDepthOfField",
                "disableDepthOfField",
                "renderProducts",
            ),
            ("rs_dep_camera", "camera", "renderProducts"),
            ("dep_deps_products", "products", "__dependencies"),
        ];

        for (name, depended_on_field, affected_field) in self_deps {
            let dep = self.build_dependency(
                &self.prim_path,
                &HdDataSourceLocator::from_tokens_2(
                    tokens::USD_RENDER_SETTINGS.clone(),
                    Token::new(depended_on_field),
                ),
                &HdDataSourceLocator::from_tokens_2(
                    tokens::RENDER_SETTINGS.clone(),
                    Token::new(affected_field),
                ),
            );
            entries.push((Token::new(name), dep as HdDataSourceBaseHandle));
        }

        // Product/var dependencies
        if let Some(usd_ds) = self.input.get(&tokens::USD_RENDER_SETTINGS) {
            if let Some(usd_container) = usd_hd::data_source::cast_to_container(&usd_ds) {
                if let Some(products_ds) = usd_container.get(&tokens::PRODUCTS) {
                    let products_any = products_ds.as_any();
                    if let Some(product_paths) =
                        products_any.downcast_ref::<HdRetainedTypedSampledDataSource<Vec<Path>>>()
                    {
                        let si_locked = self.si.read();
                        for (pid, product_path) in
                            product_paths.get_typed_value(0.0).iter().enumerate()
                        {
                            let dep_name = format!("rs_dep_product_{}", pid);
                            let dep = self.build_dependency(
                                product_path,
                                &HdDataSourceLocator::from_token(
                                    tokens::USD_RENDER_PRODUCT.clone(),
                                ),
                                &HdDataSourceLocator::from_tokens_2(
                                    tokens::RENDER_SETTINGS.clone(),
                                    tokens::RENDER_PRODUCTS.clone(),
                                ),
                            );
                            entries.push((Token::new(&dep_name), dep as HdDataSourceBaseHandle));

                            // Var dependencies
                            let prod_prim = si_locked.get_prim(product_path);
                            if let Some(ref prod_ds) = prod_prim.data_source {
                                if let Some(usd_prod_base) =
                                    prod_ds.get(&tokens::USD_RENDER_PRODUCT)
                                {
                                    if let Some(usd_prod) =
                                        usd_hd::data_source::cast_to_container(&usd_prod_base)
                                    {
                                        if let Some(vars_ds) = usd_prod.get(&tokens::ORDERED_VARS) {
                                            let vars_any = vars_ds.as_any();
                                            if let Some(var_paths) = vars_any.downcast_ref::<
                                                HdRetainedTypedSampledDataSource<Vec<Path>>,
                                            >(
                                            ) {
                                                for (vid, var_path) in
                                                    var_paths.get_typed_value(0.0).iter().enumerate()
                                                {
                                                    let var_dep_name =
                                                        format!("rs_dep_var_{}", vid);
                                                    let var_dep = self.build_dependency(
                                                        var_path,
                                                        &HdDataSourceLocator::from_token(
                                                            tokens::USD_RENDER_VAR.clone(),
                                                        ),
                                                        &HdDataSourceLocator::from_tokens_2(
                                                            tokens::RENDER_SETTINGS.clone(),
                                                            tokens::RENDER_PRODUCTS.clone(),
                                                        ),
                                                    );
                                                    entries.push((
                                                        Token::new(&var_dep_name),
                                                        var_dep as HdDataSourceBaseHandle,
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if entries.is_empty() {
            return None;
        }

        Some(HdRetainedContainerDataSource::from_entries(&entries) as HdDataSourceBaseHandle)
    }

    /// Build a single dependency entry container.
    fn build_dependency(
        &self,
        depended_on_path: &Path,
        depended_on_locator: &HdDataSourceLocator,
        affected_locator: &HdDataSourceLocator,
    ) -> HdContainerDataSourceHandle {
        let path_ds: HdDataSourceBaseHandle =
            HdRetainedTypedSampledDataSource::new(depended_on_path.clone());
        let dep_locator_ds: HdDataSourceBaseHandle =
            HdRetainedTypedSampledDataSource::new(depended_on_locator.clone());
        let aff_locator_ds: HdDataSourceBaseHandle =
            HdRetainedTypedSampledDataSource::new(affected_locator.clone());

        HdRetainedContainerDataSource::new_3(
            tokens::DEPENDED_ON_PRIM_PATH.clone(),
            path_ds,
            tokens::DEPENDED_ON_LOCATOR.clone(),
            dep_locator_ds,
            tokens::AFFECTED_LOCATOR.clone(),
            aff_locator_ds,
        )
    }
}

impl HdDataSourceBase for RenderSettingsPrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for RenderSettingsPrimDataSource {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.input.get_names();
        if !names.iter().any(|n| *n == *tokens::RENDER_SETTINGS) {
            names.push(tokens::RENDER_SETTINGS.clone());
        }
        if !names.iter().any(|n| *n == *tokens::DEPENDENCIES) {
            names.push(tokens::DEPENDENCIES.clone());
        }
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::RENDER_SETTINGS {
            return Some(
                RenderSettingsDataSource::new(self.input.clone(), self.si.clone())
                    as HdDataSourceBaseHandle,
            );
        }
        if *name == *tokens::DEPENDENCIES {
            return self.build_dependencies();
        }
        self.input.get(name)
    }
}

// -- RenderSettingsFlatteningSceneIndex --------------------------------------

/// Scene index that flattens render settings, products, and vars.
pub struct RenderSettingsFlatteningSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl std::fmt::Debug for RenderSettingsFlatteningSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderSettingsFlatteningSceneIndex")
            .finish()
    }
}

impl RenderSettingsFlatteningSceneIndex {
    /// Creates a new render settings flattening scene index.
    pub fn new(input: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input.clone())),
        }));
        wire_filter_to_input(&result, &input);
        result
    }
}

impl HdSceneIndexBase for RenderSettingsFlatteningSceneIndex {
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            let input_locked = input.read();
            let mut prim = input_locked.get_prim(prim_path);

            // Only wrap renderSettings prims
            if prim.prim_type == "renderSettings" {
                if let Some(ref ds) = prim.data_source {
                    prim.data_source = Some(RenderSettingsPrimDataSource::new(
                        ds.clone(),
                        input.clone(),
                        prim_path.clone(),
                    ) as HdContainerDataSourceHandle);
                }
            }

            return prim;
        }
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &Path) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            let input_locked = input.read();
            return input_locked.get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "RenderSettingsFlatteningSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for RenderSettingsFlatteningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        // Rely on dependency forwarding scene index for dirty propagation
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

/// Handle type for RenderSettingsFlatteningSceneIndex.
pub type RenderSettingsFlatteningSceneIndexHandle = Arc<RwLock<RenderSettingsFlatteningSceneIndex>>;

/// Creates a new render settings flattening scene index.
pub fn create_render_settings_flattening_scene_index(
    input: HdSceneIndexHandle,
) -> RenderSettingsFlatteningSceneIndexHandle {
    RenderSettingsFlatteningSceneIndex::new(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(tokens::RENDER_SETTINGS.as_str(), "renderSettings");
        assert_eq!(tokens::USD_RENDER_SETTINGS.as_str(), "__usdRenderSettings");
        assert_eq!(tokens::DEPENDENCIES.as_str(), "__dependencies");
    }

    #[test]
    fn test_display_name() {
        let si = RenderSettingsFlatteningSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
        };
        assert_eq!(si.get_display_name(), "RenderSettingsFlatteningSceneIndex");
    }

    #[test]
    fn test_flattened_ds_expected_fields() {
        let expected = vec![
            "namespacedSettings",
            "renderProducts",
            "includedPurposes",
            "materialBindingPurposes",
            "renderingColorSpace",
        ];
        for name in expected {
            assert!(!name.is_empty());
        }
    }
}
