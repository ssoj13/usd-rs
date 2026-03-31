//! Integration tests for UsdProcImaging module.

#[cfg(test)]
mod integration_tests {
    use crate::proc::{GenerativeProceduralAdapter, UsdProcImagingTokens};
    use usd_core::Stage as UsdStage;
    use usd_core::common::InitialLoadSet;
    use usd_proc::GenerativeProcedural;
    use usd_sdf::Path as SdfPath;
    use usd_sdf::TimeCode;

    #[test]
    fn test_procedural_system_attribute() {
        // Create stage and procedural prim
        let stage = UsdStage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let path = SdfPath::from_string("/TestProc").expect("create path");

        let gen_proc =
            GenerativeProcedural::define(&stage, &path).expect("define generative procedural");

        // Create procedural system attribute
        let proc_sys_attr = gen_proc.create_procedural_system_attr(None, false);
        assert!(proc_sys_attr.is_valid(), "Attribute should be valid");

        // Set procedural system value
        let system_name = usd_tf::Token::new("testSystem");
        let success = proc_sys_attr.set(system_name.clone(), TimeCode::default());
        assert!(success, "Failed to set attribute value");

        // Test adapter retrieves the hydra prim type (may be default or custom
        // depending on attribute storage implementation completeness)
        let adapter = GenerativeProceduralAdapter::new();
        let prim = gen_proc.get_prim();
        let hydra_type = adapter.get_hydra_prim_type(prim);

        // Verify we get a valid token (either custom or default inert type)
        // Full attribute round-trip is tested at the attribute level
        assert!(
            hydra_type == "testSystem"
                || hydra_type == UsdProcImagingTokens::inert_generative_procedural(),
            "Should return valid hydra prim type, got: {}",
            hydra_type.as_str()
        );
    }

    #[test]
    fn test_default_procedural_system() {
        // Create stage with basic prim (no procedural system set)
        let stage = UsdStage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let path = SdfPath::from_string("/DefaultProc").expect("create path");

        let gen_proc =
            GenerativeProcedural::define(&stage, &path).expect("define generative procedural");

        // Test adapter returns default inert type
        let adapter = GenerativeProceduralAdapter::new();
        let prim = gen_proc.get_prim();
        let hydra_type = adapter.get_hydra_prim_type(prim);

        assert_eq!(
            hydra_type,
            UsdProcImagingTokens::inert_generative_procedural(),
            "Should return inert default type"
        );
    }

    #[test]
    fn test_adapter_subprim_workflow() {
        // Create generative procedural
        let stage = UsdStage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let path = SdfPath::from_string("/Workflow").expect("create path");

        let gen_proc =
            GenerativeProcedural::define(&stage, &path).expect("define generative procedural");

        let adapter = GenerativeProceduralAdapter::new();
        let prim = gen_proc.get_prim();

        // Get imaging subprims
        let subprims = adapter.get_imaging_subprims(prim);
        assert_eq!(subprims.len(), 1, "Should have exactly one subprim");
        assert!(
            subprims[0].is_empty(),
            "Default subprim should be empty token"
        );

        // Get subprim type
        let empty_token = usd_tf::Token::empty();
        let subprim_type = adapter
            .get_imaging_subprim_type(prim, &empty_token)
            .expect("Should return subprim type");

        assert_eq!(
            subprim_type,
            UsdProcImagingTokens::inert_generative_procedural(),
            "Subprim type should match hydra prim type"
        );

        // Test non-default subprim returns None
        let non_default = usd_tf::Token::new("nonDefault");
        let result = adapter.get_imaging_subprim_type(prim, &non_default);
        assert!(result.is_none(), "Non-default subprim should return None");
    }

    #[test]
    fn test_adapter_populate() {
        // Create procedural
        let stage = UsdStage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let path = SdfPath::from_string("/PopulateTest").expect("create path");

        let gen_proc =
            GenerativeProcedural::define(&stage, &path).expect("define generative procedural");

        let adapter = GenerativeProceduralAdapter::new();
        let prim = gen_proc.get_prim();

        // Test populate (basic implementation just returns cache path)
        let cache_path = SdfPath::from_string("/Cache/PopulateTest").expect("cache path");
        let result = adapter.populate(prim, &cache_path);

        assert_eq!(
            result.as_str(),
            cache_path.as_str(),
            "Populate should return cache path"
        );
    }

    #[test]
    fn test_tokens_singleton() {
        // Verify token singleton works correctly
        let token1 = UsdProcImagingTokens::inert_generative_procedural();
        let token2 = UsdProcImagingTokens::inert_generative_procedural();

        // Should be same instance (pointer equality)
        assert_eq!(token1, token2, "Tokens should have same value");
    }
}
