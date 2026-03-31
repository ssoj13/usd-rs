# USD-SDR Full Parity Report

Reference: `_ref/OpenUSD/pxr/usd/sdr/`
Rust: `crates/usd/usd-sdr/src/`

## File Mapping (C++ -> Rust)

| C++ File | Rust File | Status |
|---|---|---|
| declare.h/cpp | declare.rs | OK |
| shaderNode.h/cpp | shader_node.rs | OK |
| shaderProperty.h/cpp | shader_property.rs | OK |
| registry.h/cpp | registry.rs | OK |
| shaderNodeMetadata.h/cpp | shader_node_metadata.rs | OK |
| shaderPropertyMetadata.h/cpp | shader_property_metadata.rs | OK |
| shaderMetadataHelpers.h/cpp | shader_metadata_helpers.rs | OK |
| sdfTypeIndicator.h/cpp | sdf_type_indicator.rs | **BUG** |
| discoveryPlugin.h/cpp | discovery_plugin.rs | OK |
| shaderNodeDiscoveryResult.h | discovery_result.rs | OK |
| filesystemDiscovery.h/cpp | filesystem_discovery.rs | OK |
| filesystemDiscoveryHelpers.h/cpp | filesystem_discovery_helpers.rs | OK |
| parserPlugin.h/cpp | parser_plugin.rs | OK |
| shaderNodeQuery.h/cpp | shader_node_query.rs | OK |
| shaderNodeQueryUtils.h/cpp | shader_node_query_utils.rs | OK |
| (no equivalent) | tokens.rs | Rust-specific (replaces TF macros) |
| (no equivalent) | args_parser.rs | Rust-specific parser |
| (no equivalent) | osl_parser.rs | Rust-specific parser |
| (no equivalent) | sdrosl_parser.rs | Rust-specific parser |
| (no equivalent) | usd_shaders.rs | Rust-specific built-in shaders |

## BUG: SdrSdfTypeIndicator::PartialEq

**File:** `sdf_type_indicator.rs`

C++ `operator==` only compares `_sdfType` and `_sdrType`:
```cpp
bool operator==(const SdrSdfTypeIndicator &rhs) const {
    return _sdfType == rhs._sdfType && _sdrType == rhs._sdrType;
}
```

Rust also compares `has_sdf_type_mapping` which is WRONG:
```rust
fn eq(&self, other: &Self) -> bool {
    self.sdf_type == other.sdf_type
        && self.sdr_type == other.sdr_type
        && self.has_sdf_type_mapping == other.has_sdf_type_mapping // BUG
}
```

**Fix:** Remove `has_sdf_type_mapping` from PartialEq.

## Tests to Port

### 1. testSdrVersion (from testSdrVersion.py)
- Invalid version creation and bool checks
- Default version marking
- Relational operators (==, !=, <, <=, >, >=) comprehensive
- String representation
- String suffix
- **Status:** Partially covered in declare.rs tests, needs expansion

### 2. testSdrFilesystemDiscovery (from testSdrFilesystemDiscovery.py)
- Discovery with search paths and allowed extensions
- Nested directory discovery
- Duplicate handling (same name different type)
- SdrFsHelpersSplitShaderIdentifier parsing
- SdrFsHelpersDiscoverFiles URI matching
- **Status:** Needs full port
- **Test data needed:** testSdrFilesystemDiscovery.testenv/ (all .args, .osl, .oso files)

### 3. testSdrRegistry (from testSdrRegistry.py)
- Source type deduplication across discovery plugins
- GetShaderNodesByFamily parsing
- GetShaderNodeNames vs GetShaderNodeIdentifiers (discovery-only vs parsed)
- GetShaderNodeByName with type priority
- GetShaderNodeByIdentifier with type priority
- GetShaderNodeFromAsset with subIdentifier
- USD encoding version 0 vs 1 type mapping differences
- ParseSdfValue basic sanity
- **Status:** Partially covered, needs expansion
- **Test data needed:** testSdrRegistry/ (TestNodeSourceAsset.osl/oso)

### 4. testSdrShaderNodeQuery (from testSdrShaderNodeQuery.py)
- SelectDistinct single/multiple keys
- NodeValueIs / NodeValueIsNot filtering
- NodeValueIsIn / NodeValueIsNotIn
- NodeHasValueFor / NodeHasNoValueFor
- CustomFilter functions
- GetShaderNodesByValues grouping
- GetAllShaderNodes from query
- GetStringifiedValues
- **Status:** Needs full port
- **Test data needed:** testSdrShaderNodeQuery.testenv/ (SimpleNodes.usda, testDummy.glslfx)

### 5. testSdrShownIfConversion (from testSdrShownIfConversion.py)
- conditionalVis metadata -> shownIf expression conversion
- Basic and recursive metadata
- Preservation of explicit shownIf expressions
- **Status:** Needs full port
- **Test data needed:** testSdrShownIfConversion.testenv/ (TestConversion.usda, TestPassThrough.usda, testDummy.glslfx)

### 6. testSdrParseValue (from testSdrParseValue.cpp)
- Int property parsing (valid/invalid)
- Int array property parsing (fixed size 2)
- Terminal property parsing (Token type)
- String property parsing (empty, special chars, newlines)
- Asset property parsing (paths, relative, empty)
- **Status:** Needs full port

## Test Data Files to Copy

From `_ref/OpenUSD/pxr/usd/sdr/testenv/`:

### testSdrFilesystemDiscovery.testenv/
- TestNodeARGS.args
- TestNodeOSL.osl, TestNodeOSL.oso
- TestNodeSameName.args, TestNodeSameName.osl, TestNodeSameName.oso
- Primvar.args
- Primvar_float.args, Primvar_float_3.args, Primvar_float_3_4.args
- Primvar_float2.args, Primvar_float2_3.args, Primvar_float2_3_4.args
- nested/NestedTestARGS.args, nested/NestedTestOSL.osl, nested/NestedTestOSL.oso

### testSdrShaderNodeQuery.testenv/
- SimpleNodes.usda
- testDummy.glslfx

### testSdrShownIfConversion.testenv/
- TestConversion.usda
- TestPassThrough.usda
- testDummy.glslfx

### testSdrRegistry/
- TestNodeSourceAsset.osl
- TestNodeSourceAsset.oso

## Functional Parity Summary

### declare.rs - COMPLETE
All types, SdrVersion, SdrVersionFilter match C++.

### shader_node.rs - COMPLETE
All methods match C++ including PostProcessProperties, InitializePrimvars, ComputePages, GetAllVstructNames, CheckPropertyCompliance, GetDataForKey.

### shader_property.rs - NEEDS VERIFICATION
Need to verify:
- Type conversion tables (_GetTokenTypeToSdfType, _GetTokenTypeToSdfArrayType)
- Role-based conversion (_GetConvertedSdrTypes)
- Encoding 0 vs Encoding 1 differences
- _ConformSdrDefaultValue / _ConformSdfTypeDefaultValue
- CanConnectTo full logic

### registry.rs - COMPLETE (with architectural differences)
- Uses RwLock instead of std::mutex (appropriate for Rust)
- Plugin discovery is manual instead of via libplug (by design)
- SdrRegistry_ValidateProperty exists in C++ but unclear if ported
- RunQuery implemented

### sdf_type_indicator.rs - BUG (PartialEq too strict)

### discovery_result.rs - COMPLETE
All fields match C++ struct. Extra convenience methods (minimal, from_source_code) are Rust additions.

### tokens.rs - COMPLETE
All token definitions match C++ TF_DECLARE_PUBLIC_TOKENS macros.
