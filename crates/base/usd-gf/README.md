# usd-gf -- Graphics Foundation

Rust port of OpenUSD `pxr/base/gf`.

GF is the math library of OpenUSD. It provides all linear algebra, geometric primitives, and color operations used throughout the pipeline:

- **Vectors** -- Vec2/3/4 in double, float, half, int variants. Dot, cross, normalize, project, complement, slerp, orthogonalize
- **Matrices** -- Matrix2/3/4 in double and float. Inverse, determinant, transpose, decompose (scale/rotate/translate), LookAt, factor
- **Quaternions** -- Quatd/f/h with slerp, DualQuat for rigid transforms, legacy Quaternion type
- **Rotation** -- Axis-angle with full Euler decomposition (`decompose_rotation`), `RotateOntoProjected`, bidirectional Matrix3d/Matrix4d conversion
- **Ranges** -- Range1/2/3 d/f for bounding intervals, intersection, union, containment
- **Geometric primitives** -- Plane (with LSQ `fit_plane_to_points`), Ray, Line, LineSeg, Line2d, LineSeg2d, BBox3d, Rect2i
- **Closest points** -- All 6 overloads: line-line, line-seg, seg-seg (3D and 2D)
- **Frustum & Camera** -- Physical camera model with focal length, aperture, FOV; frustum with 6-plane extraction and lazy caching
- **Color** -- GfColor with full ColorSpace support (18 named spaces), Planckian locus, HSV, chromaticity
- **Half** -- IEEE 754 half-precision float with full ILMBase lookup tables
- **Transform** -- Composed scale/rotate/translate with pivot and center
- **Utilities** -- Gamma (apply/display), homogeneous projection, numeric cast, smooth_step/smooth_ramp

Reference: `_ref/OpenUSD/pxr/base/gf`

## Parity Status

All public C++ APIs have Rust equivalents. Verified header-by-header against the reference.

---

### Vector Types

| C++ Header | Rust File | Methods | Status |
|---|---|---|---|
| vec2d.h / vec2f.h / vec2h.h / vec2i.h | vec2.rs | 53 methods (Vec2d, Vec2f, Vec2h, Vec2i) — dot, length, normalize, cross, comp_mult, comp_div, GetProjection, GetComplement, operators, Hash, Display | 100% |
| vec3d.h / vec3f.h / vec3h.h / vec3i.h | vec3.rs | 64 methods (Vec3d, Vec3f, Vec3h, Vec3i) — cross, OrthogonalizeBasis, BuildOrthonormalFrame, Slerp + all Vec2 methods | 100% |
| vec4d.h / vec4f.h / vec4h.h / vec4i.h | vec4.rs | 56 methods (Vec4d, Vec4f, Vec4h, Vec4i) — all Vec2 methods + homogeneous operations | 100% |

### Matrix Types

| C++ Header | Rust File | Methods | Status |
|---|---|---|---|
| matrix2d.h / matrix2f.h | matrix2.rs | 38 methods (Matrix2d, Matrix2f) — Set, SetIdentity, SetZero, SetDiagonal, Get, GetTranspose, GetInverse, GetDeterminant, operators, Hash | 100% |
| matrix3d.h / matrix3f.h | matrix3.rs | 44 methods (Matrix3d, Matrix3f) — SetRotate, ExtractRotation, SetScale, Orthonormalize, GetHandedness + all Matrix2 methods | 100% |
| matrix4d.h / matrix4f.h | matrix4.rs | 68 methods (Matrix4d, Matrix4f) — SetTransform, SetLookAt, SetTranslate, SetRotate, SetScale, RemoveScaleShear, ExtractTranslation, ExtractRotation, Factor (5 components: r,s,u,t,p), HasOrthogonalRows + all Matrix3 methods | 100% |
| matrixData.h | N/A | Template helper, not needed in Rust | N/A |

### Quaternion Types

| C++ Header | Rust File | Methods | Status |
|---|---|---|---|
| quatd.h / quatf.h / quath.h | quat.rs | 30 methods (Quatd, Quatf, Quath) — GetReal, GetImaginary, GetConjugate, GetInverse, GetLength, GetNormalized, Normalize, Transform, Slerp, GfDot, operators | 100% |
| quaternion.h | quaternion.rs | 19 methods (Quaternion legacy type) — GetReal, GetImaginary, GetLength, GetNormalized, Normalize, GetConjugate, GetInverse, Transform, operators | 100% |
| dualQuatd.h / dualQuatf.h / dualQuath.h | dual_quat.rs | 27 methods (DualQuatd, DualQuatf, DualQuath) — GetReal, GetDual, SetReal, SetDual, GetZero, GetIdentity, GetLength, GetNormalized, Normalize, GetConjugate, GetInverse, SetTranslation, GetTranslation, Transform, GfDot, operators, cross-precision From conversions | 100% |

### Range Types

| C++ Header | Rust File | Methods | Status |
|---|---|---|---|
| range1d.h / range1f.h | range.rs | Range1d, Range1f — GetMin, GetMax, GetSize, GetMidpoint, SetMin, SetMax, Contains, IsInside, IsOutside, IntersectWith, UnionWith, GetDistanceSquared, operators, Hash | 100% |
| range2d.h / range2f.h | range.rs | Range2d, Range2f — same as Range1 + 2D operations | 100% |
| range3d.h / range3f.h | range.rs | Range3d, Range3f — same as Range1 + 3D operations | 100% |

Total: 72 methods across all range types.

### Geometric Primitives

| C++ Header | Rust File | Methods | Status |
|---|---|---|---|
| plane.h | plane.rs | 15 methods — Set, GetNormal, GetDistanceFromOrigin, GetDistance, Project, Reorient, IntersectsPositiveHalfSpace, Transform, FitToPoints, operators | 100% |
| ray.h | ray.rs | 18 methods — new, from_endpoints, start_point, direction, point, transform, find_closest_point, intersect_plane, intersect_range, intersect_bbox, intersect_sphere, intersect_cylinder, intersect_cone, intersect_triangle, operators. + find_closest_points_ray_line, find_closest_points_ray_line_seg | 100% |
| line.h | line.rs | 19 methods — Line + LineSeg: Set, GetPoint, GetDirection, FindClosestPoint, GetLength. + GfFindClosestPoints(line, line), (line, seg), (seg, seg), (ray, line), (ray, seg) | 100% |
| lineSeg.h | line.rs | Merged into line.rs | 100% |
| line2d.h | line2d.rs | 19 methods — Line2d + LineSeg2d | 100% |
| lineSeg2d.h | line2d.rs | Merged into line2d.rs | 100% |
| bbox3d.h | bbox3d.rs | 20 methods — Set, SetMatrix, SetRange, GetRange, GetBox, GetMatrix, GetInverseMatrix, SetHasZeroAreaPrimitives, HasZeroAreaPrimitives, GetVolume, Transform, ComputeAlignedRange, ComputeAlignedBox, Combine, ComputeCentroid, operators, Hash | 100% |
| rect2i.h | rect.rs | 29 methods — Rect2i: Set, GetMinX/Y, GetMaxX/Y, GetWidth/Height, GetArea, GetCenter, Contains, Intersect, Union, Translate, operators | 100% |
| size2.h / size3.h | size.rs | 17 methods — Size2, Size3 | 100% |

### Complex Types

| C++ Header | Rust File | Methods | Status |
|---|---|---|---|
| frustum.h | frustum.rs | 46 methods — +GetReferencePlaneDepth, +SetPerspectiveFromFovHeight; ComputeAspectRatio uses fabs | 100% |
| camera.h | camera.rs | 38 methods — Set/Get Transform, Projection, FocalLength, HorizontalAperture, VerticalAperture, ApertureOffsets, SetPerspectiveFromAspectRatioAndFieldOfView, SetOrthographicFromAspectRatioAndSize, SetFromViewAndProjectionMatrix, ClippingRange, ClippingPlanes, FStop, FocusDistance, AspectRatio, FieldOfView, GetFrustum, operators | 100% |
| rotation.h | rotation.rs | 17 methods + 3 static — SetAxisAngle, SetQuat, SetQuaternion, SetRotateInto, SetIdentity, GetAxis, GetAngle, GetQuat, GetQuaternion, GetInverse, Decompose, DecomposeRotation (static), MatchClosestEulerRotation (static), RotateOntoProjected (static), TransformDir, operators (*, *=, /=, ==, !=), Hash, Display | 100% |
| transform.h | transform.rs | 22 methods — Set (2x and 3x arg order), SetMatrix, SetIdentity, Set/Get Scale, PivotOrientation, ScaleOrientation, Rotation, PivotPosition, Center, Translation, GetMatrix, operators (*=, *, ==, !=), Display | 100% |
| interval.h | interval.rs | 28 methods — SetMin/Max, GetMin/Max, GetSize, Contains, IsEmpty, Intersects, Intersect, operators, Hash | 100% |
| multiInterval.h | multi_interval.rs | 25 methods — Add, Remove, Contains, IsEmpty, GetSize, GetBounds, GetComplement, Intersect, operators, iterator | 100% |

### Color Types

| C++ Header | Rust File | Methods | Status |
|---|---|---|---|
| color.h | color.rs | 38 methods — Constructors (default, colorSpace, rgb+colorSpace, convert), SetFromPlanckianLocus, GetRGB, GetColorSpace, chromaticity, set_from_chromaticity, GfIsClose, operators, Display + bonus (HSV, hex, lerp, clamp, multiply, add, scale) | 100% |
| colorSpace.h | color_space.rs | 23 methods — from name/token/primaries/matrix, IsValid, GetName, GetRGBToXYZ, GetRGBToRGB, GetGamma, GetLinearBias, GetTransferFunctionParams, GetPrimariesAndWhitePoint, Convert, ConvertRGBSpan, ConvertRGBASpan, operators. All 18 named spaces supported (LinearAP1, LinearAP0, LinearRec709, LinearP3D65, LinearRec2020, LinearAdobeRGB, LinearCIEXYZD65, SRGBRec709, G22Rec709, G18Rec709, SRGBAP1, G22AP1, SRGBP3D65, G22AdobeRGB, Identity, Data, Raw, Unknown) with correct RGB-to-XYZ matrices | 100% |

### Utilities

| C++ Header | Rust File | Methods | Status |
|---|---|---|---|
| math.h | math.rs | 38 methods — Abs, Sqr, Sgn, Sqrt, Exp, Log, Floor, Ceil, Round, Pow, Clamp, Mod, Lerp, Min, Max, DegreesToRadians, RadiansToDegrees, IsClose, DotProduct, CompMult, CompDiv, Normalize | 100% |
| limits.h | limits.rs | 5 constants — MIN_VECTOR_LENGTH, MIN_ORTHO_TOL, etc. | 100% |
| traits.h | traits.rs | 15 traits/impls — Scalar, IsFloatingPoint, IsGfVec, IsGfMatrix, etc. | 100% |
| half.h | half.rs | 46 methods — from_f32, to_f32, from_f64, to_f64, from_bits, bits, is_finite, is_normalized, is_denormalized, is_zero, is_nan, is_infinite, is_negative, is_positive, abs, signum, round, min, max, clamp, q_nan, s_nan, Hash (uses bits), num_traits::Float full impl, arithmetic operators, From conversions | 100% |
| gamma.h | gamma.rs | 6 methods + Vec3h/Vec4h — ApplyGamma (all types incl half), Convert*, GetDisplayGamma | 100% |
| homogeneous.h | homogeneous.rs | 6 methods — Project, GetHomogenized, IsClose | 100% |
| numericCast.h | numeric_cast.rs | GfIntegerCompareLess → integer_compare_less, GfNumericCast → numeric_cast, GfNumericCastFailureType → NumericCastFailure. Supports int, float, bool, GfHalf | 100% |

### Not Ported (not needed in Rust)

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| declare.h | C++ forward declarations |
| ostreamHelpers.h | Rust `Display` trait |
| ilmbase_*.h/cpp | Internal half-float implementation (we use `half` crate) |
| nc/nanocolor.h/c | Nanocolor C implementation (we have native Rust color math) |
| pyBufferUtils.h | Python bindings |
| wrap*.cpp | Python bindings |
| *.template.h | C++ template files (Rust uses generics) |
| overview.dox | Doxygen docs |
| colorSpace_data.h | Internal C++ data (baked into Rust constants) |

---

## API Differences from C++

| C++ API | Rust Equivalent | Rationale |
|---|---|---|
| `GfMatrix4d::GetInverse(double* det)` | `inverse() -> Option<Matrix4d>` | Rust idiom: `Option` instead of out-param |
| `GfMatrix4d::Set*(...)` mutating methods | `from_scale()`, `from_rotation()` constructors | Immutable-first Rust style |
| `GfFindClosestPoints(Line, Line, ...)` free fn | `find_closest_points_line_line()` module fn | Same semantics, snake_case |
| `GfFitPlaneToPoints(vector<Vec3d>, Plane*)` | `fit_plane_to_points(&[Vec3d]) -> Option<Plane>` | `Option` instead of bool + out-param |
| `GfRotation::DecomposeRotation(Matrix4d, ...)` 5 out-params | `decompose_rotation(mat, tw, fb, lr, sw, ...)` `Option<&mut f64>` | Same algorithm, Rust out-param style |
| `GfFrustum` atomic pointer caching | `RwLock`-based caching | Thread-safe, no raw pointer manipulation |
| ILMBase `half` embedded C code | `half.rs` with full lookup tables | Pure Rust, no C dependency |
| NanoColor C library | Native Rust color math | Pure Rust, no C dependency |

---

## Implementation Notes

### FitPlaneToPoints
Least-squares plane fitting via scatter matrix + Cramer's rule. Tries 3 fixed-axis systems (a=1, b=1, c=1), picks best determinant. Returns `None` for degenerate/collinear inputs.

### Rotation Decomposition
Full port of C++ `DecomposeRotation` with `RotateOntoProjected` x3 iterations, `MatchClosestEulerRotation` (up to 4 candidates), `ShiftGimbalLock`, and `PiShiftAngle` per-component.

### FindClosestPoints (6 overloads)
All 6 combinations implemented matching C++ variable naming (`a,b,c,d,e,f` coefficients, `1e-6` threshold). 3D: line-line, line-seg, seg-seg. 2D: line2d-line2d, line2d-seg2d, seg2d-seg2d.

### Frustum Plane Caching
C++ uses `std::atomic<GfPlane*>` with double-checked locking. Rust uses `RwLock<Option<Vec<Plane>>>` — safer, no raw pointer handling.

### Half-Precision Float
Full IEEE 754 half with embedded ILMBase eLUT (18K table). Implements `num_traits::Float`. All special cases handled (NaN, Inf, subnormals).

Verified 2026-02-22.
