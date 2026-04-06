# Gf Module — Python API Inventory

## module.cpp TF_WRAP entries
BBox3d, Color, ColorSpace, DualQuatd, DualQuatf, DualQuath,
Frustum, Gamma, Half, Homogeneous, Interval, Limits, Line, LineSeg, Math, MultiInterval,
Matrix2d, Matrix2f, Matrix3d, Matrix3f, Matrix4f, Matrix4d,
Plane, Quatd, Quatf, Quath, Quaternion, Ray,
Range1f, Range1d, Range2f, Range2d, Range3f, Range3d,
Rect2i, Rotation, Size2, Size3,
Vec2h, Vec2f, Vec2d, Vec2i, Vec3h, Vec3f, Vec3d, Vec3i, Vec4h, Vec4f, Vec4d, Vec4i,
Transform, Camera

## __init__.py
Minimal: `Tf.PreparePythonModule()`

---

## VEC TYPES (12 types: Vec2/3/4 × d/f/h/i)

### Common API (all vec types, from wrapVec.template.cpp)
- **Constructors**: `()`, `(scalar)`, `(x,y[,z[,w]])`, `(tuple/list)`, `(other_precision_vec)`
- **Methods**: `Axis(i)` (static), `XAxis/YAxis/ZAxis/WAxis()` (static), `GetDot`, `GetComplement`, `GetLength`, `GetNormalized`, `GetProjection`, `Normalize`
- **Vec3 extra**: `GetCross`, `OrthogonalizeBasis` (static), `BuildOrthonormalFrame`
- **Properties**: `dimension` (r/o), `__isGfVec` (r/o)
- **Indexing**: `__getitem__` (int/slice), `__setitem__` (int/slice), `__contains__`, `__len__`
- **Operators**: `==`, `!=`, `+=`, `-=`, `*=`, `/=`, `+`, `-`, `*`, `/`, `__truediv__`, `__itruediv__`, unary `-`
- **Vec3 extra op**: `^` (cross product)
- **Module functions** (float types only): `Dot`, `CompDiv`, `CompMult`, `GetLength`, `GetNormalized`, `GetProjection`, `GetComplement`, `IsClose`, `Normalize`
- **Vec3 module extra**: `Cross`, `Slerp`
- **Registrations**: buffer_protocol, FromPythonTuple, TfPyContainerConversions, vector<> converter
- **Special**: `__repr__`, `__hash__`, pickle
- **Note**: Vec*i types have NO module-level functions and NO cross-precision constructors

### Type list
| Type | Scalar | Cross-precision init |
|------|--------|---------------------|
| Vec2d | double | from Vec2f, Vec2h, Vec2i |
| Vec2f | float | from Vec2d, Vec2h, Vec2i |
| Vec2h | GfHalf | from Vec2d, Vec2f, Vec2i |
| Vec2i | int | none |
| Vec3d | double | from Vec3f, Vec3h, Vec3i |
| Vec3f | float | from Vec3d, Vec3h, Vec3i |
| Vec3h | GfHalf | from Vec3d, Vec3f, Vec3i |
| Vec3i | int | none |
| Vec4d | double | from Vec4f, Vec4h, Vec4i |
| Vec4f | float | from Vec4d, Vec4h, Vec4i |
| Vec4h | GfHalf | from Vec4d, Vec4f, Vec4i |
| Vec4i | int | none |

---

## MATRIX TYPES (6 types: Matrix2/3/4 × d/f)

### Common API (all matrix types)
- **Constructors**: `()`, `(scalar)`, `(other_precision)`, row-by-row, flat 4/9/16 args
- **Methods**: `SetZero`, `SetIdentity`, `Invert`, `GetDeterminant`, `GetInverse`, `GetTranspose`, `Transpose`, `Get`, `Set`, `GetRow`, `SetRow`, `GetColumn`, `SetColumn`
- **Matrix3 extra**: `RotationMatrix`, `ScaleMatrix`, `AlignZAxisWithVector`, `ExtractRotation`
- **Matrix4 extra**: `ExtractTranslation`, `ExtractRotation`, `SetTranslateAndScale`, `RotationMatrix`, `ScaleMatrix`, `TranslationMatrix`, `LookAt`, `Perspective`, `Orthographic`, `RemoveScaleShear`, `GetHandedness`
- **Indexing**: `__getitem__` (row→col 2D), `__setitem__`, `__contains__`, `__len__`
- **Operators**: `==`, `!=`, `*=`, `/=`, `+`, `-`, `*`, `/`, `__truediv__`, `__itruediv__`, unary `-`, matrix×matrix
- **Registrations**: buffer_protocol (2D shape), to_python_converter, TfPyContainerConversions
- **Special**: `__repr__`, `__hash__`, pickle

---

## QUATERNION TYPES (7 types: Quatd/f/h + Quaternion + DualQuatd/f/h)

### Quat API (Quatd, Quatf, Quath, Quaternion)
- **Module functions**: `Dot`, `Slerp`
- **Methods**: `GetZero` (static), `GetIdentity` (static), `GetReal`, `SetReal`, `GetImaginary`, `SetImaginary`, `GetInverse`, `GetLength`, `GetNormalized`, `Normalize`
- **Properties**: `real` (r/w), `imaginary` (r/w)
- **Operators**: `==`, `!=`, `+=`, `-=`, `*=`, `/=`, `+`, `-`, `*`, `/`, `__truediv__`, `__itruediv__`, double×quat
- **Special**: `__repr__`, `__hash__`

### DualQuat API (DualQuatd, DualQuatf, DualQuath)
- **Methods**: `GetZero` (static), `GetIdentity` (static), `GetReal`, `SetReal`, `GetDual`, `SetDual`, `GetLength`, `GetNormalized`, `Normalize`, `GetConjugate`, `GetInverse`, `SetTranslation`, `GetTranslation`, `Transform`
- **Properties**: `real` (r/w), `dual` (r/w)
- **Operators**: same as Quat

---

## RANGE TYPES (6 types: Range1/2/3 × d/f)

### Common API
- **Methods**: `Contains(x)`, `Contains(Range)`, `In`, `GetFullInterval` (static), `Intersects`, `IsEmpty`, `IsFinite`, `IsMaxFinite`, `IsMinFinite`, `IsMaxClosed`, `IsMaxOpen`, `IsMinClosed`, `IsMinOpen`, `GetMax`, `GetMin`, `GetSize`, `SetMax(scalar)`, `SetMax(Range)`, `SetMin(scalar)`, `SetMin(Range)`
- **Properties**: `min` (r/w), `max` (r/w), `minClosed` (r/o), `maxClosed` (r/o), `minOpen` (r/o), `maxOpen` (r/o), `minFinite` (r/o), `maxFinite` (r/o), `finite` (r/o), `isEmpty` (r/o), `size` (r/o)
- **Operators**: `==`, `!=`, `&=`, `&`, unary `-`, implicit bool

---

## GEOMETRIC TYPES

### wrapBBox3d.cpp → GfBBox3d
- **Methods**: `Set`, `GetBox`, `GetRange`, `GetInverseMatrix`, `GetMatrix`, `SetHasZeroAreaPrimitives`, `SetMatrix`, `SetRange`, `Transform`, `ComputeAlignedBox`, `ComputeAlignedRange`, `ComputeCentroid`, `Combine` (static), `GetVolume`, `HasZeroAreaPrimitives`
- **Properties**: `box` (r/w), `matrix` (r/w), `hasZeroAreaPrimitives` (r/w)
- **Operators**: `==`, `!=`, `str`
- **Constructors**: `()`, `(Range3d)`, `(Range3d, Matrix4d)`

### wrapInterval.cpp → GfInterval
- Same API as Range types + `IsEmpty`, `IsFinite` etc.
- **Operators**: `==`, `!=`, `&=`, `&`

### wrapMultiInterval.cpp → GfMultiInterval
- **Methods**: `Contains`, `In`, `IsEmpty`, `IsFinite`, `Intersects`, `GetSize`, `GetFullInterval` (static), `Union`, `Complement`, `Intersection`, `SetMin`, `SetMax`, `GetMin`, `GetMax`
- **Operators**: `==`, `!=`, `&=`, `&`, `|=`, `|`, `-`, `-=`

### wrapLine.cpp → GfLine
- **Methods**: `Set`, `GetPoint`, `GetDirection`
- **Operators**: `==`, `!=`, `str`, `repr`

### wrapLineSeg.cpp → GfLineSeg
- **Methods**: `GetDirection`, `GetLength`, `GetPoint`
- **Properties**: `direction` (r/o), `length` (r/o)
- **Operators**: `==`, `!=`, `str`

### wrapRay.cpp → GfRay
- **Module functions**: `FindClosestPoints` (2 overloads)
- **Methods**: `SetPointAndDirection`, `SetEnds`, `GetPoint`, `FindClosestPoint`, `Transform`, `Intersect` (7 overloads: triangle, plane, Range3d, BBox3d, sphere, cylinder, cone)
- **Properties**: `startPoint` (r/w), `direction` (r/w)
- **Operators**: `==`, `!=`, `str`, `repr`

### wrapPlane.cpp → GfPlane
- **Methods**: `Set`, `GetNormal`, `GetDistance`, `GetDistanceFromOrigin`, `SetNormal`, `SetDistance`, `GetDistance(point)`
- **Operators**: `==`, `!=`

### wrapRotation.cpp → GfRotation
- **Methods**: `SetAxisAngle`, `SetQuat`, `SetQuaternion`, `SetRotateInto`, `SetIdentity`, `GetAxis`, `GetAngle`, `GetQuaternion`, `GetQuat`, `GetInverse`, `Decompose`, `DecomposeRotation3` (static), `DecomposeRotation` (static), `MatchClosestEulerRotation` (static)
- **Properties**: `axis` (r/w), `angle` (r/w)
- **Operators**: `==`, `!=`, `*`, `/`, `+=`, `-=`, `*=`, `/=`
- **Constructors**: `()`, `(Vec3d, double)`, `(Quaternion)`, `(Quatd)`, `(Vec3d, Vec3d)`

### wrapTransform.cpp → GfTransform
- **Methods**: `SetIdentity`, `Set`, `GetMatrix`, `SetMatrix`, `GetInverse`, `GetInverseMatrix`, `GetTranslation`, `SetTranslation`, `GetRotation`, `SetRotation`, `GetScale`, `SetScale`, `GetPivot`, `SetPivot`, `GetPivotPosition`, `SetPivotPosition`, `GetScalePivot`, `SetScalePivot`, `GetScalePivotPosition`, `SetScalePivotPosition`
- **Operators**: `==`, `!=`, `*`

### wrapCamera.cpp → GfCamera
- **Methods**: `GetFieldOfView`, `SetPerspectiveFromAspectRatioAndFieldOfView`, `SetOrthographicFromAspectRatioAndSize`, `SetFromViewAndProjectionMatrix`
- **Properties**: `transform` (r/w), `projection` (r/w), `horizontalAperture` (r/w), `verticalAperture` (r/w), `horizontalApertureOffset` (r/w), `verticalApertureOffset` (r/w), `aspectRatio` (r/w), `focalLength` (r/w), `clippingRange` (r/w), `clippingPlanes` (r/w), `frustum` (r/o), `fStop` (r/w), `focusDistance` (r/w), `horizontalFieldOfView` (r/o), `verticalFieldOfView` (r/o)

### wrapFrustum.cpp → GfFrustum
- **Methods**: `SetPerspective` (3 overloads), `GetPerspective`, `GetFOV`, `SetOrthographic`, `GetOrthographic`, `GetPosition`, `SetPosition`, `GetRotation`, `SetRotation`, `SetPositionAndRotationFromMatrix`, `GetWindow`, `SetWindow`, `GetNearFar`, `SetNearFar`, `GetViewDistance`, `SetViewDistance`, `FitToSphere`, `Transform`, `ComputeViewDirection`
- **Operators**: `==`, `!=`

### wrapRect2i.cpp → GfRect2i
- **Methods**: `SetMin`, `SetMax`, `SetSize`, `GetMin`, `GetMax`, `GetSize`, `GetWidth`, `GetHeight`, `Contains`, `GetArea`
- **Properties**: `min` (r/w), `max` (r/w), `size` (r/w), `width` (r/o), `height` (r/o), `area` (r/o)
- **Operators**: `==`, `!=`

### wrapSize2.cpp → GfSize2, wrapSize3.cpp → GfSize3
- **Methods**: `Get`, `Set`
- **Indexing**: `__getitem__`, `__setitem__`, `__contains__`, `__len__`
- **Operators**: `==`, `!=`, `*=`, `/=`, `*`, `/`, `+`, `-`, unary `-`

---

## UTILITY (no class, module-level functions)

### wrapColor.cpp → GfColor
- `SetFromPlanckianLocus`, `GetRGB`, `GetColorSpace`, `__repr__`

### wrapColorSpace.cpp → GfColorSpace
- `GetName`, `ConvertRGBSpan`, `ConvertRGBASpan`, `Convert`, `GetRGBToXYZ`, `GetGamma`, `GetLinearBias`, `GetTransferFunctionParams`, `GetPrimariesAndWhitePoint`, `IsValid`, `__repr__`

### wrapHalf.cpp — GfHalf ↔ Python float converters
### wrapGamma.cpp — gamma/linear conversion functions
### wrapMath.cpp — math utilities (Sqr, Pow, Abs, Sqrt, etc.)
### wrapHomogeneous.cpp — homogeneous coordinate utilities
### wrapLimits.cpp — float/double limits

---

## Universal registrations (all types)
- buffer_protocol (vec, matrix)
- to_python_converter (vector<T>)
- TfPyContainerConversions (from_python_sequence)
- pickle support
- TfTypePythonClass()
- __repr__, __hash__

## Total: ~60 wrap files, 12 vec + 6 matrix + 7 quat + 6 range + ~15 geometric + ~5 utility = ~51 types
