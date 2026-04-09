# pxr (usd-pyo3) — паритет с OpenUSD Python API

**Референс C++ / Python:** `C:\projects\projects.rust.cg\usd-refs\OpenUSD`  
**Крейт:** `crates/usd-pyo3`  
**Пакет:** `pxr` (`import pxr as pxr`)  
**Правило:** любая публичная поведенческая деталь — сверка с `wrap*.cpp` / `tf*.py` / тестами `pxr/usd/...` в референсе. Отклонения считаются багами, пока не задокументированы как намеренные ограничения порта.

**Коррективы порта (Rust + Python):** намеренное отличие от Pixar допустимо только с **ID** в [`PYTHON_API_DEVIATIONS.md`](PYTHON_API_DEVIATIONS.md) и краткой причиной (архитектура PyO3, заглушка до появления Rust-API, и т.д.). Иначе цель — **поведенческий паритет** с `usd-refs/OpenUSD` и портированными тестами в `crates/usd-pyo3/tests/`. Карта C++→Rust: `docs/src/appendix/cpp-rust-mapping.md`; общий указатель на C++ дерево: [`STRUCTURE.md`](../STRUCTURE.md) (раздел *OpenUSD C++ reference*).

**Рабочая память (очередь wrap, процесс):** [`md/PYTHON_API_WORK.md`](PYTHON_API_WORK.md).  
**Полный реестр отклонений (ID, приоритеты, что ещё не как в референсе):** [`md/PYTHON_API_DEVIATIONS.md`](PYTHON_API_DEVIATIONS.md) — построчные таблицы: **`wrapBBoxCache.cpp`** (§2), **`wrapXformCache.cpp`** (§3), **`wrapBasisCurves.cpp`** (§4), **`wrapBoundable.cpp`** (§5), **`wrapCamera.cpp`** (§6), **`wrapCapsule.cpp` / `wrapCapsule_1.cpp`** (§7), **`wrapCone.cpp`** (§8), **`wrapConstraintTarget.cpp`** (§9), **`wrapCube.cpp`** (§10), **пакет статических API схем** (§20), **`Sdr.shaderParserTestUtils`** (§21, G4 — нативная реализация вместо upstream `.py`).

---

## Инварианты

1. **Vt.Value из Python:** однородные последовательности должны становиться **типизированными** `VtArray` / `vector`, а не `vector[VtValue]`. Иначе `Attribute.Get` / геометрия (`PointInstancer`, примитивы) не читают значения. Реализация: `crates/usd-pyo3/src/vt.rs` — `py_to_value`.
2. **Usd.Attribute.Set / Create*Attr(default):** используют тот же `py_to_value` (`usd.rs` делегирует в `vt::py_to_value`).
3. **Время:** `Usd.TimeCode` и `float` → SDF time через `usd.rs::tc_from_py_sdf` для схемных методов с `TimeCode`.
4. **Имена методов:** CamelCase как в C++ (`#[pyo3(name = "...")]`).
5. **Один нативный модуль:** вспомогательные тестовые API вроде **`pxr.Sdr.shaderParserTestUtils`** не подгружают Python через `compile`/`exec` и не дублируют upstream-`.py` внутри wheel — реализация в Rust (`sdr_shader_parser_test_utils.rs`), см. **DEVIATIONS G4 / §21**.

---

## Сделано (журнал)

| Дата | Изменение |
|------|-----------|
| 2026-04-07 | **`UsdGeom.Cube`:** `GetSchemaAttributeNames`, `GetExtentAttr`/`CreateExtentAttr`. Док.: **`wrapCube.cpp`** → §10 (CU-*). |
| 2026-04-07 | **`UsdGeom.ConstraintTarget`:** полный класс из `wrapConstraintTarget.cpp` (`Get`/`Set`/`ComputeInWorldSpace`/`GetConstraintAttrName`/…). Док.: §9 (CT-*). |
| 2026-04-07 | **`UsdGeom.Cone`:** `GetSchemaAttributeNames`, `GetExtentAttr`/`CreateExtentAttr`. Док.: **`wrapCone.cpp`** → §8 (CO-*). |
| 2026-04-07 | **`UsdGeom.Capsule` / `Capsule_1`:** `GetSchemaAttributeNames`, `GetExtentAttr`/`CreateExtentAttr`; `Capsule1::get_schema_attribute_names` в `usd-geom`. Док.: §7 (CAP-*). |
| 2026-04-07 | **`UsdGeom.Camera`:** `GetSchemaAttributeNames`; атрибуты exposure* (Get/Create); `GetCamera` → **`Gf.Camera`**; `SetFromCamera`; `ComputeLinearExposureScale`. Док.: **`wrapCamera.cpp`** → §6 (CA-*). |
| 2026-04-07 | **`UsdGeom.Boundable`:** `Get`, `GetSchemaAttributeNames`, `ComputeExtent`, static `ComputeExtentFromPlugins` (+ optional `Gf.Matrix4d`); `usd-geom`: `Boundable::compute_extent_from_plugins`. Док.: **`wrapBoundable.cpp`** → `PYTHON_API_DEVIATIONS.md` §5 (BO-*). |
| 2026-04-07 | Документация: построчная сверка **`wrapBasisCurves.cpp`** → `PYTHON_API_DEVIATIONS.md` §4 (BC-*); нумерация разделов DEVIATIONS сдвинута. |
| 2026-04-08 | `vt::py_to_value`: списки/кортежи `Gf.Vec3f` / `(f,f,f)` → `VtArray<Vec3f>` (`from_no_hash`); списки целых → `Vec<i32>` в `Value`; обёртки `Vt.Vec3fArray` / `Vt.IntArray`; `usd::py_to_value` = делегирование в `vt`. |
| 2026-04-08 | **Порядок в `py_to_value`:** однородные `int`/`Vec3f` последовательности **до** `Vec<f64>` — иначе `[0]` превращался в `[0.0]` и ломал `protoIndices`. |
| 2026-04-08 | `UsdGeom.PointInstancer`: `ComputeInstanceTransformsAtTime(s)`, `ComputeInstanceTransformsAtTimes`, `ComputeExtentAtTime(s)`, `CreatePrototypesRel` / `GetPrototypesRel` → `Usd.Relationship`, `Create*Attr(default, writeSparsely)`, константы `ApplyMask` / `IgnoreMask`. |
| 2026-04-08 | `UsdRelationship::pack` для межмодульной сборки. |
| 2026-04-08 | `UsdGeom` все `Get`/`Define(stage, path)`: `path` — `str` **или** `Sdf.Path` (`parse_path_py`), как в C++. |
| 2026-04-08 | `UsdGeom.BBoxCache`: время `Usd.TimeCode`/`float` через `tc_from_py_sdf`; kwargs как в C++: `includedPurposes`, `useExtentsHint`, `ignoreVisibility`. |
| 2026-04-08 | `UsdGeom.BBoxCache`: экспорт `ComputePointInstance*Bound(s)` → `Gf.BBox3d`; `instance_ids` — list/tuple/`range` (`geom.rs`). Остаётся паритет по `ComputeWorldBound`→Gf, Relative/Untransformed/Overrides, purposes/base time — см. `PYTHON_API_DEVIATIONS.md` §2. |
| 2026-04-08 | **Пакет схем UsdGeom (`geom.rs`):** `GetSchemaAttributeNames(includeInherited=True)` с пробросом в `usd_geom` для **Imageable, Xformable, Xform, Boundable, Scope, Gprim, Mesh, Sphere, Cube, Cone, Cylinder, Cylinder_1, Capsule, Capsule_1, Plane, PointBased, Points, Curves, BasisCurves, NurbsCurves, HermiteCurves, NurbsPatch, TetMesh, PointInstancer, Subset, Camera**, а также **VisibilityAPI**, **ModelAPI**. Исправлена передача **`include_inherited`** (раньше часто игнорировался) у **Imageable, Xformable, Scope, Mesh**. **GetExtentAttr/CreateExtentAttr** добавлены/доступны для сферы, цилиндров, плоскости (и ранее для куба/конуса/капсул). Док.: **`PYTHON_API_DEVIATIONS.md` §20**. |
| 2026-04-08 | **`UsdGeom.PointBased`** и **`UsdGeom.Mesh`:** `ComputePointsAtTime(time, baseTime)`, `ComputePointsAtTimes(times, baseTime)` → списки `Gf.Vec3f` (пустой список при неуспехе core, как в ряде других Compute* в биндинге). Референс: `UsdGeomPointBased::ComputePointsAtTime` / `ComputePointsAtTimes` (`point_based.rs`). |
| 2026-04-09 | **`UsdGeom.Xformable`:** делегирование **`UsdGeomImageable`** — `GetVisibilityAttr` / `CreateVisibilityAttr`, purpose attrs, `ComputeVisibility` / `ComputePurpose`, `MakeVisible` / `MakeInvisible`, `ComputeWorldBound` / `ComputeLocalBound`, `GetOrderedPurposeTokens` (как `wrapXformable.cpp`, `bases<UsdGeomImageable>`). |
| 2026-04-09 | **`UsdGeom.Curves`:** `SetWidthsInterpolation`, `GetCurveCount(time)` (`usd_geom::Curves`); static `ComputeExtent` уже был. |
| 2026-04-09 | **`UsdGeom.BasisCurves`:** пять `Compute*` из `wrapBasisCurves.cpp`; **`GetSchemaAttributeNames`**; **`CreateTypeAttr` / `CreateBasisAttr` / `CreateWrapAttr`** с kwargs как в C++. |
| 2026-04-09 | **`UsdGeom.BasisCurves`:** методы базового **`UsdGeom.Curves`** (static `ComputeExtent`, curve vertex counts attrs, `SetWidthsInterpolation`, `GetCurveCount`). **`UsdGeom.Curves`:** kwargs на **`CreateCurveVertexCountsAttr`**. |
| 2026-04-09 | **`UsdGeom.Mesh`:** kwargs на **`Create*Attr`** (как `wrapMesh.cpp`); **`GetFaceCount`** с временем как `UsdTimeCode`; **`SHARPNESS_INFINITE`** — атрибут класса (тест `testUsdGeomConsts`). |
| 2026-04-09 | **`NurbsCurves` / `HermiteCurves`:** наследуемые методы **`UsdGeom.Curves`** + kwargs; Nurbs: **`PointWeights`**. **`Sphere`:** **`Extent`**, kwargs **Radius**/**Extent**, **`GetSchemaAttributeNames`**. |
| 2026-04-09 | **`pxr.Sdr.shaderParserTestUtils`:** полностью в Rust (`sdr_shader_parser_test_utils.rs`), без embedded Python; **`PyShaderNode` / `PyShaderProperty`** — `pub(crate) inner` для доступа из хелперов. Док.: **DEVIATIONS G4, §21**. |

---

## Очередь (без пропусков, по модулям)

### База: `Tf`, `Vt`, `Gf`

- [ ] `Tf`: иерархия `Notice`, хэш/сравнение `Token`, `Type` — дотошно к `wrapTf*.cpp`.
- [x] `Vt`: типизированный `py_to_value` для массивов (см. журнал); при необходимости расширить: `Vec3d[]`, `Quatf[]`, тензоры под тесты.
- [ ] `Gf`: `Gf.IsClose` для всех типов из тестов референса.

### `Sdf`

- [ ] `Sdf.FileFormat`, `CopySpec`, `ZipFileWriter`, `VariableExpression.*`, `Layer.SetDetachedLayerRules`, `BatchNamespaceEdit`, `PathListOp.explicitItems` writable — по `wrapSdf*.cpp` и `testSdf*.py`.

### `Usd` (core)

- [ ] `Attribute.Set/Get` — все комбинации time samples / metadata из тестов.
- [ ] Путь: везде принимать `Sdf.Path` и `str` (сделано для `UsdGeom.Get`/`Define`; остальные модули — `usd.rs`, `shade.rs`, …).
- [ ] `Stage.OpenMasked` с `Layer`, `Layer.relocates`, и т.д. по тестам.

### `UsdGeom`

- [x] `BBoxCache`: point-instance методы (`ComputePointInstance*`) — см. журнал; полный список отличий от `wrapBBoxCache.cpp` — **`md/PYTHON_API_DEVIATIONS.md` §2** (World/Local bound как flat list, отсутствующие API, base time, …).
- [ ] `Gf.Matrix4d.SetTranslate` / аналоги: кортежи **целых** `(2,2,2)` — привести к референсу (см. **DEVIATIONS §11 XF1**).
- [ ] Делегирование **Xformable** / **Imageable** для всех конкретных схем (Mesh, Sphere, Camera, …), не только `Xform`.
- [x] `PointBased.ComputePointsAtTime` / `ComputePointsAtTimes` на **`PointBased`** и **`Mesh`** (см. журнал 2026-04-08); остальные схемы с point-based — по тестам при необходимости.
- [ ] `Boundable.ComputeExtentFromPlugins` (доп. перегрузки/семантика), `BBoxCache.__new__(includedPurposes=...)` vs полный ctor, `PrimvarsAPI.CreatePrimvar` с `ValueTypeName`, и т.п. — по списку тестов `testUsdGeom*.py`.

### Прочее

- [ ] `UsdShade` / `UsdLux` — модули, от которых зависят коллекции тестов (`Sdr`, импорты).
- [x] `Sdr.shaderParserTestUtils` — нативные функции паритета с OpenUSD (см. **DEVIATIONS §21**); pytest по OSL / shader-def тестам — при появлении в `crates/usd-pyo3/tests/` или прогоне против дерева OpenUSD.
- [ ] Полный прогон `pytest crates/usd-pyo3/tests` после каждого крупного блока; обновлять счётчики в этом файле.

---

## Как проверять

```bash
python -m maturin develop -m crates/usd-pyo3/Cargo.toml --release
python -m pytest crates/usd-pyo3/tests/<path> -q --tb=no
cargo check -p usd-pyo3
```

---

## Известные риски

- **PyO3 / kwargs:** имена вида `doProtoXforms` в C++ — в Rust часто `snake_case`; позиционные аргументы совпадают. При необходимости полного совпадения имён kwargs — отдельная задача (обёртка или расширение PyO3).
- **`from_prim_auto` / dummy stage:** для `PyAttribute` при создании из схемы всё ещё используется заглушка stage — для полного паритета нужна привязка к реальному `Arc<Stage>` из примитива (см. `usd.rs` / `PyAttribute::from_attr`).
