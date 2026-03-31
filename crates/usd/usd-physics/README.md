# UsdPhysics Module - Physics Schemas

Rust port of OpenUSD `pxr/usd/usdPhysics`. Rigid body dynamics, joints, and collision.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdPhysics/*.h` on 2026-02-08.

---

### Scene & Bodies

| C++ Header | Rust File | Status |
|---|---|---|
| scene.h | scene.rs | 100% |
| rigidBodyAPI.h | rigid_body_api.rs | 100% |
| massAPI.h | mass_api.rs | 100% |
| massProperties.h | mass_properties.rs | 100% |
| collisionAPI.h | collision_api.rs | 100% |
| collisionGroup.h | collision_group.rs | 100% |
| meshCollisionAPI.h | mesh_collision_api.rs | 100% |
| materialAPI.h | material_api.rs | 100% |

### Joints

| C++ Header | Rust File | Status |
|---|---|---|
| joint.h | joint.rs | 100% |
| fixedJoint.h | fixed_joint.rs | 100% |
| revoluteJoint.h | revolute_joint.rs | 100% |
| prismaticJoint.h | prismatic_joint.rs | 100% |
| sphericalJoint.h | spherical_joint.rs | 100% |
| distanceJoint.h | distance_joint.rs | 100% |
| limitAPI.h | limit_api.rs | 100% |
| driveAPI.h | drive_api.rs | 100% |

### Utilities

| C++ Header | Rust File | Status |
|---|---|---|
| articulationRootAPI.h | articulation_root_api.rs | 100% |
| filteredPairsAPI.h | filtered_pairs_api.rs | 100% |
| metrics.h | metrics.rs | 100% |
| parseDesc.h | parse_desc.rs | 100% |
| parseUtils.h | parse_utils.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |

---

## Summary

**UsdPhysics module: 100% API parity with OpenUSD C++ reference.**

All 22 public C++ headers fully covered. 23 Rust source files. 0 API gaps.

Verified 2026-02-08.
