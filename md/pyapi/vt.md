# Vt Module — Python API Inventory

## module.cpp TF_WRAP entries
Array, ArrayDualQuaternion, ArrayFloat, ArrayIntegral, ArrayMatrix,
ArrayQuaternion, ArrayRange, ArrayString, ArrayToken, ArrayVec,
Dictionary, Value, ValueRef
+ Vt_AddBufferProtocolSupportToVtArrays()

## __init__.py
- `_CopyArrayFromBufferFuncs()` — attaches `.FromBuffer()` and `.FromNumpy()` static methods to all VtArray classes

---

## wrapArray.h → VtArray<T> TEMPLATE (applies to ALL array types)

### Constructors
- `__init__()` — empty
- `__init__(values)` — from sequence (list/tuple/VtArray)
- `__init__(size, values)` — with explicit size + tiling

### Indexing/Slicing
- `__getitem__(ellipsis)` — all elements
- `__getitem__(int)` — single element
- `__getitem__(slice)` — slice
- `__setitem__(ellipsis, value)` — set all
- `__setitem__(int, value)` — set single
- `__setitem__(slice, value)` — set slice with broadcasting

### Iteration/Length
- `__len__()` — size
- `__iter__()` — iterator

### String
- `__repr__()` — type, size, elements
- `__str__()` — via TfStringify

### Comparison
- `__eq__`, `__ne__`

### Numeric (conditional per type)
- `__add__` / `__radd__` (ADDITION_OPERATOR)
- `__sub__` / `__rsub__` (SUBTRACTION_OPERATOR)
- `__mul__` / `__rmul__` (MULTIPLICATION_OPERATOR)
- `__div__` / `__rdiv__` (DIVISION_OPERATOR)
- `__mod__` / `__rmod__` (MOD_OPERATOR)
- `__neg__` (UNARY_NEG_OPERATOR)
- `self * double`, `double * self` (DOUBLE_MULT_OPERATOR)
- `self / double` (DOUBLE_DIV_OPERATOR)

### Implicit Conversions
- VtArray<T> → TfSpan<T>, TfSpan<const T>
- Python sequences → VtArray<T>

---

## wrapArrayEdit.h → VtArrayEdit<T> + VtArrayEditBuilder<T>

### VtArrayEdit<T>
- `__eq__`, `__ne__`, `__hash__`
- `IsIdentity()`
- `ComposeOver(weaker: ArrayEdit)`, `ComposeOver(weaker: Array)`

### VtArrayEditBuilder<T>
- `Write(elem, index)`, `WriteRef(srcIdx, dstIdx)`
- `Insert(elem, index)`, `InsertRef(srcIdx, dstIdx)`
- `Prepend(elem)`, `PrependRef(srcIdx)`
- `Append(elem)`, `AppendRef(srcIdx)`
- `EraseRef(index)`
- `MinSize(size)`, `MinSize(size, fill)`
- `MaxSize(size)`, `SetSize(size)`, `SetSize(size, fill)`
- `FinalizeAndReset()`
- Static: `Optimize(edit)`

---

## wrapArrayVec.cpp — VtVec*Array + VtVec*ArrayEdit
Types: Vec2/3/4 × d/f/h/i = 12 array types + 12 edit types
Operators: ADDITION, SUBTRACTION, UNARY_NEG, DOUBLE_MULT

## wrapArrayMatrix.cpp — VtMatrix*Array + VtMatrix*ArrayEdit
Types: Matrix2/3/4 × d/f = 6 array types + 6 edit types
Operators: ALL NUMERIC + DOUBLE_MULT

## wrapArrayQuaternion.cpp — VtQuat*Array + VtQuat*ArrayEdit
Types: Quath/f/d + Quaternion = 4 array types + 4 edit types
Operators: ADDITION, SUBTRACTION, MULTIPLICATION, DOUBLE_MULT, DOUBLE_DIV

## wrapArrayFloat.cpp — VtDoubleArray, VtFloatArray, VtHalfArray + edits
Operators: ALL NUMERIC

## wrapArrayIntegral.cpp — VtBool/Char/UChar/Short/UShort/Int/UInt/Int64/UInt64 Array + edits
Operators: ALL NUMERIC + MOD

## wrapArrayString.cpp — VtStringArray + VtStringArrayEdit
Operators: ADDITION only

## wrapArrayToken.cpp — VtTokenArray + VtTokenArrayEdit
Operators: none

## wrapArrayRange.cpp — VtRange1/2/3 d/f + Interval + Rect2i Array + edits
Operators: ADDITION

## wrapArrayDualQuaternion.cpp — VtDualQuath/f/d Array + edits
Operators: ADDITION, SUBTRACTION, MULTIPLICATION, DOUBLE_MULT, DOUBLE_DIV

---

## wrapValue.cpp → VtValue

### Auto conversions Python→VtValue
- None → empty VtValue
- bool → bool
- int → int/long/int64_t/uint64_t (range-dependent)
- float → double
- bytes/str → std::string
- sequences → VtArray<T> (via registry)
- TfToken → TfToken
- generic → TfPyObjWrapper

### _ValueWrapper static factories (explicit type)
- Bool(v), UChar(v), Short(v), UShort(v), Int(v), UInt(v)
- Long(v), ULong(v), Int64(v), UInt64(v)
- Half(v), Float(v), Double(v), Token(v)

### _ValueWrapper methods
- `__eq__`, `__ne__`, `__str__`, `__repr__`

---

## wrapDictionary.cpp → VtDictionary ↔ Python dict

- VtDictionary → dict (recursive)
- dict → VtDictionary (recursive, keys must be strings)
- vector<VtDictionary> ↔ list
- vector<VtValue> ↔ list
- Nested dict/list support

## wrapValueRef.cpp → VtValueRef
- `_test_ValueRefFromPython(valueRef)` — test helper

---

## Total: 80+ VtArray<T> instantiations, 80+ VtArrayEdit<T>, ~20 methods/ops per type
## Buffer protocol (numpy): ALL numeric + vec + matrix + quat + dualquat + range types
