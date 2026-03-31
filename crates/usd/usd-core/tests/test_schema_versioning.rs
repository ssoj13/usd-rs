//! Port of testUsdSchemaVersioning.py from OpenUSD
//! Tests versioned schema types and version policy logic.

mod common;

#[test]
#[ignore = "Needs test plugin with versioned schema types (92KB test)"]
fn schema_versioning() {
    common::setup();
    // C++ registers testUsdSchemaVersioning plugin with multiple versions
    // of typed and API schemas, tests FindSchemaInfo, IsConcreteSchemaKind,
    // VersionPolicy logic, IsAllowedAPISchemaInstanceName, etc.
}
