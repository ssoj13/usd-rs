# Рабочая память: OpenUSD Python-паритет (pxr)

**Назначение:** короткий контекст между сессиями; детали отклонений — в [`PYTHON_API_DEVIATIONS.md`](PYTHON_API_DEVIATIONS.md), инварианты — в [`PYTHON_API_PARITY.md`](PYTHON_API_PARITY.md).

## Процесс (зафиксировано)

- Короткие циклы: **один `wrap*.cpp` / одна схема** → строки в DEVIATIONS + **по возможности сразу код** (P0/P1), не откладывать всё на «после полного аудита».
- После блока: `cargo check -p usd-pyo3` (и pytest по затронутым тестам, если менялось поведение).

## Сейчас

| Поле | Значение |
|------|----------|
| Последний крупный блок | **`pxr.Sdr.shaderParserTestUtils`:** перенос с embedded Python на **чистый Rust** (`sdr_shader_parser_test_utils.rs`); **G4** + **DEVIATIONS §21**; удалён `embed/shader_parser_test_utils.py`. До этого: **`wrapNurbsPatch.cpp`:** делегаты **`PointBased`**, kwargs **`Create*Attr`**, trim-curves, **`GetSchemaAttributeNames`**. |
| Следующий приоритет (код) | Kwargs **`Create*Attr`** на остальных схемах по DEVIATIONS §4 / §10; следующий **`wrap*.cpp`** по очереди аудита UsdGeom. |
| Параллельно | Kwargs **`Create*Attr`** на остальных схемах (см. DEVIATIONS §4 / §10). |
| PointBased | В Python: **`ComputePointsAtTime` / `ComputePointsAtTimes`** (`geom.rs`) — см. журнал PARITY. |

## Последнее обновление

- **2026-04-09 (ё):** **Sdr / Plug / реестр:** `FindTypeByName` — встроенные имена парсеров через `Tf::declare_by_name`; **`oso` → `OslParserPlugin`**, **`sdrOsl` → JSON** (`sdrosl_parser` без дубля `oso`); `NodeDiscoveryResult` / `GetShaderNodeByIdentifier` — паритет kwargs и optional `type_priority`. Прогон: `pytest .../testOslParser.py` при `OPENUSD_SRC_ROOT` — setup проходит; полный зелёный паритет с upstream OSL может требовать доработок **osl-rs / метаданных** (см. **DEVIATIONS §21** хвост).
- **2026-04-09 (е):** **`Sdr.shaderParserTestUtils`:** реализация в Rust, политика одного `.pyd`; документация **PYTHON_API_DEVIATIONS G4 / §21**, **PYTHON_API_PARITY** (инвариант 5, журнал, очередь). `cargo check -p usd-pyo3`; pytest по парсерам — при наличии тестов / `OPENUSD` дерева.
- **2026-04-09 (д):** **`UsdGeom.NurbsCurves`:** `GetPointWeightsAttr` / `CreatePointWeightsAttr`; kwargs на Order/Knots/Ranges; делегаты **`Curves`**; **`GetSchemaAttributeNames`**. **`UsdGeom.HermiteCurves`:** делегаты **`Curves`**; kwargs **`CreateTangentsAttr`**; **`GetSchemaAttributeNames`**. **`UsdGeom.Sphere`:** **`GetExtentAttr`** / **`CreateExtentAttr`**; kwargs radius/extent; **`GetSchemaAttributeNames`**.
- **2026-04-09 (г):** **`UsdGeom.Mesh`** (`wrapMesh.cpp`): kwargs на **Create** для топологии / subdiv / creases / PointBased vel·norm·accel; **`GetFaceCount(time)`**; **`SHARPNESS_INFINITE`** как class attr.
- **2026-04-09 (в):** **`UsdGeom.BasisCurves`:** наследуемые от **`UsdGeom.Curves`** — static **`ComputeExtent`**, **`GetCurveVertexCountsAttr`**, **`CreateCurveVertexCountsAttr`** (kwargs), **`SetWidthsInterpolation`**, **`GetCurveCount`**. **`UsdGeom.Curves`:** **`CreateCurveVertexCountsAttr`** с kwargs.
- **2026-04-09 (б):** **`UsdGeom.BasisCurves`:** `ComputeInterpolationForSize`, `ComputeUniformDataSize`, `ComputeVaryingDataSize`, `ComputeVertexDataSize`, `ComputeSegmentCounts`; **`GetSchemaAttributeNames`**; **`CreateTypeAttr` / `CreateBasisAttr` / `CreateWrapAttr`** с kwargs `default_value`, `write_sparsely`.
- **2026-04-09:** **XF3:** на **`UsdGeom.Xformable`** добавлен полный API **Imageable** (как `bases<UsdGeomImageable>` в C++). **`UsdGeom.Curves`:** `SetWidthsInterpolation`, `GetCurveCount(time)`; DEVIATIONS §4.7 обновлён под `wrapCurves.cpp`.
- **2026-04-08:** Обновлены WORK / PARITY / DEVIATIONS §20 под пакет схем; добавлены `UsdGeom.PointBased` / `UsdGeom.Mesh` — `ComputePointsAtTime`, `ComputePointsAtTimes`.
- **2026-04-07:** `wrapCube.cpp` — код + §10; блок XF в DEVIATIONS — **§11**.
