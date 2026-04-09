# Реестр отклонений pxr от OpenUSD Python (полный снимок для исправлений)

**Референс:** `C:\projects\projects.rust.cg\usd-refs\OpenUSD` (`pxr` / `wrap*.cpp`, `tf/*.py`, тесты `pxr/...`).  
**Порт:** `crates/usd-pyo3`, пакет `pxr`.  
**Связанные документы:** `md/PYTHON_API_PARITY.md` (инварианты и журнал), `md/PYTHON_API_WORK.md` (рабочая память сессии: очередь, процесс), `TODO.md` (счётчики тестов).

**Легенда приоритета:** **P0** — ломает тесты / явно неверное поведение; **P1** — отсутствует метод или семантика ≠ C++; **P2** — заглушка / частичная реализация; **P3** — косметика (repr, тип возврата).

---

## 1. Глобальные архитектурные долги

| ID | Тема | Суть отклонения | Где в коде | Приоритет |
|----|------|-----------------|------------|-----------|
| G1 | `PyPrim` / `PyAttribute` и stage | `from_prim_auto` / `from_attr` держат **фиктивный** `Arc<Stage>` для GC; логический `Prim` несёт свой `Weak<Stage>`, но публичный API Python может рассчитывать на «тот же» stage, что у обёртки — расхождение с инвариантом «атрибут привязан к живой стадии». | `crates/usd-pyo3/src/usd.rs` (`PyPrim`, `PyAttribute::from_attr`) | P1 |
| G2 | Имена kwargs | PyO3: часть C++ camelCase kwargs требует `#[pyo3(signature)]` + `allow(non_snake_case)`; полное совпадение имён со всеми тестами референса не гарантировано. | разные `*.rs` | P2 |
| G3 | Покрытие тестами | По `TODO.md`: ~23% pytest проходят (~887 failed + errors в подмножестве модулей); любой модуль может содержать незадокументированные расхождения до полного прогона. | `crates/usd-pyo3/tests/**` | — |
| G4 | Один нативный модуль (`_usd`), без встроенного Python для этого API | В OpenUSD **`pxr.usd.sdr.shaderParserTestUtils`** поставляется как **`.py`** (`pxr/usd/sdr/shaderParserTestUtils.py`). В usd-rs те же имена и проверки реализованы **в Rust** (`#[pyfunction]`, подмодуль `pxr.Sdr.shaderParserTestUtils`), без `compile`/`exec` и без копии скрипта в колесе. Поведение — паритет с тестами парсеров (OSL / Args / USD shader defs), не с текстом `.py`. | `crates/usd-pyo3/src/sdr_shader_parser_test_utils.rs`, регистрация в `sdr.rs` | — |

---

## 2. `UsdGeom.BBoxCache` — построчная сверка с `wrapBBoxCache.cpp`

**Источник референса:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapBBoxCache.cpp` (класс `BBoxCache`, строки ~105–165).  
**Реализация pxr:** `crates/usd-pyo3/src/geom.rs` (`PyBBoxCache`), низлежащий тип `usd_geom::BBoxCache`.

**Легенда статуса:** **OK** — есть и имя совпадает; **PARTIAL** — есть, но сигнатура/тип/семантика отличаются от pxr; **MISSING** — нет в Python; **N/A** — вопрос не к Python-обёртке.

### 2.1 Конструктор

| wrap (строки) | C++ `boost::python` | pxr | Статус / отклонение |
|---------------|---------------------|--------|---------------------|
| L107–109 | `init<UsdTimeCode, TfTokenVector, optional<bool,bool>>(time, includedPurposes, useExtentsHint, ignoreVisibility)` | `#[new]` `time: Usd.TimeCode\|float` → `tc_from_py_sdf`, `includedPurposes: list[str]`, `useExtentsHint`, `ignoreVisibility` | **PARTIAL (P2):** в C++ второй аргумент — `TfTokenVector`; у нас `Vec<String>` → `Token` — эквивалентно. Время на стороне Python — через обёртку `Usd.TimeCode`, не «сырой» только `TfTokenVector` тип как в C++ — для тестов обычно достаточно. |

### 2.2 Методы (порядок как в `wrapBBoxCache.cpp`)

| wrap (строки) | Метод C++ / Python pxr | pxr | Статус |
|---------------|--------------------------|--------|--------|
| L110 | `ComputeWorldBound(prim)` → `GfBBox3d` | `ComputeWorldBound` → **`list[float]`** (6: min xyz, max xyz) через `bbox_to_flat` | **PARTIAL (P1)** — тип возврата не `Gf.BBox3d`. |
| L111–116 | `ComputeWorldBoundWithOverrides(prim, pathsToSkip, primOverride, ctmOverrides)` | — | **MISSING (P1)** |
| L117 | `ComputeLocalBound(prim)` → `GfBBox3d` | `ComputeLocalBound` → **`list[float]`** (6) | **PARTIAL (P1)** |
| L118–119 | `ComputeRelativeBound(prim, relativeRootPrim)` | — | **MISSING (P1)** |
| L120–121 | `ComputeUntransformedBound(prim)` | — | **MISSING (P1)** |
| L122–125 | `ComputeUntransformedBound(prim, pathsToSkip, ctmOverrides)` перегрузка | — | **MISSING (P1)** |
| L126–128 | `ComputePointInstanceWorldBounds(instancer, instanceIds)` → `list` или **`None`** при неуспехе C++ | `ComputePointInstanceWorldBounds` → `list[Gf.BBox3d]`; при ошибке парсинга id — исключение; **нет `None`** при «false» от core | **PARTIAL (P2)** — тип бокса OK; семантика **`None`** как в C++ не воспроизведена (см. §2.4). |
| L129–131 | `ComputePointInstanceWorldBound(instancer, instanceId)` | есть → `Gf.BBox3d` | **OK** |
| L132–135 | `ComputePointInstanceRelativeBounds(instancer, instanceIds, relativeToAncestorPrim)` | есть | **PARTIAL (P2)** — как с batch world |
| L136–139 | `ComputePointInstanceRelativeBound(instancer, instanceId, relativeToAncestorPrim)` | есть | **OK** |
| L140–142 | `ComputePointInstanceLocalBounds(instancer, instanceIds)` | есть | **PARTIAL (P2)** — batch/`None` |
| L143–145 | `ComputePointInstanceLocalBound(instancer, instanceId)` | есть | **OK** |
| L146–148 | `ComputePointInstanceUntransformedBounds(instancer, instanceIds)` | есть | **PARTIAL (P2)** — batch/`None` |
| L149–151 | `ComputePointInstanceUntransformedBound(instancer, instanceId)` | есть | **OK** |
| L153 | `Clear()` | `Clear` | **OK** |
| L154–155 | `SetIncludedPurposes(includedPurposes)` | — | **MISSING (P1)** |
| L156–157 | `GetIncludedPurposes()` → list | — | **MISSING (P1)** |
| L158 | `SetTime(time)` — `UsdTimeCode` | `SetTime(time: float)` | **PARTIAL (P2)** — нужен `Usd.TimeCode` / согласованность с `GetTime` |
| L159 | `GetTime()` → `UsdTimeCode` | `GetTime()` → **`float`** (`value()`) | **PARTIAL (P2)** — для `Default()` теряется тип-обёртка |
| L160 | `SetBaseTime(time)` | — | **MISSING (P1)** |
| L161 | `GetBaseTime()` | — | **MISSING (P1)** |
| L162 | `HasBaseTime()` | — | **MISSING (P1)** |
| L163 | `ClearBaseTime()` | — | **MISSING (P1)** |
| L164 | `GetUseExtentsHint()` | `GetUseExtentsHint` | **OK** |

### 2.3 Что есть в C++ `UsdGeomBBoxCache` / `bboxCache.h`, но нет в `wrapBBoxCache.cpp`

| API | Примечание |
|-----|------------|
| `GetIgnoreVisibility()` | В **заголовке** C++ есть (`bboxCache.h`); в **wrapBBoxCache.cpp** к классу **не** добавлен `.def("GetIgnoreVisibility", ...)`. Официальный Python pxr может не экспортировать это — перед добавлением в pxr сверить с установленным у пользователя `pxr`. |

### 2.4 Поведение batch PointInstancer (статические helpers L19–91)

В C++ при `!self.ComputePointInstance*(...)` обёртка возвращает **пустой Python-объект** (`None`). В Rust-порте `usd_geom` методы возвращают **`Vec<BBox3d>`** без `bool` успеха; в Python мы всегда отдаём список (возможны пустые/дефолтные боксы). **Отклонение:** нет зеркала **`None`** при провале C++-уровня — уточнить по `usd_geom::BBoxCache` и при необходимости маппить пустой/ошибочный результат в `None`.

### 2.5 Сводные ID (для коммитов)

| ID | Суть |
|----|------|
| BB-WL | `ComputeWorldBound` / `ComputeLocalBound` → вернуть **`Gf.BBox3d`**, не flat list |
| BB-REL | Добавить `ComputeRelativeBound` |
| BB-UT | Добавить обе перегрузки `ComputeUntransformedBound` |
| BB-WO | Добавить `ComputeWorldBoundWithOverrides` |
| BB-PURP | `SetIncludedPurposes` / `GetIncludedPurposes` |
| BB-BASE | `SetBaseTime` / `GetBaseTime` / `HasBaseTime` / `ClearBaseTime` |
| BB-TIME | `SetTime` / `GetTime` принимать/возвращать **`Usd.TimeCode`** (или совместимую обёртку), не только `float` |
| BB-PI-NONE | Batch `ComputePointInstance*Bounds` → **`None`** при неуспехе, как в C++ |
| BB-OK-PI | Одиночные `ComputePointInstance*Bound` и ctor — **OK** по имени и типу бокса |

---

## 3. `UsdGeom.XformCache` — построчная сверка с `wrapXformCache.cpp`

**Источник:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapXformCache.cpp` (функция `wrapUsdGeomXformCache`, строки ~45–64).  
**pxr:** `crates/usd-pyo3/src/geom.rs` (`PyXformCache`), низлежащий `usd_geom::XformCache`.

> В **заголовке** `xformCache.h` есть дополнительные методы (`IsAttributeIncludedInLocalTransform`, `TransformMightBeTimeVarying`, `GetResetXformStack`, …); в официальном **Python wrap** они **не** экспортируются — ниже только то, что объявлено в `wrapXformCache.cpp`.

### 3.1 Методы (порядок как в wrap)

| wrap (строки) | C++ / pxr Python | pxr | Статус |
|---------------|------------------|--------|--------|
| L49–50 | `__init__(time: UsdTimeCode)` | `XformCache(time=None)` → `Option<f64>` → внутренний `TimeCode` | **PARTIAL (P2)** — нет явного `Usd.TimeCode` в сигнатуре; `None` → default time |
| L51–52 | `GetLocalToWorldTransform(prim)` → `GfMatrix4d` | `list[float]` длины 16 (`mat4_to_flat`) | **PARTIAL (P1)** — не `Gf.Matrix4d` |
| L53–54 | `GetParentToWorldTransform(prim)` → `GfMatrix4d` | `list[float]` ×16 | **PARTIAL (P1)** |
| L55–56 | `GetLocalTransformation(prim)` → `(matrix, resetsXformStack)` | — | **MISSING (P1)** |
| L57–58 | `ComputeRelativeTransform(prim, ancestor)` → `(matrix, resetXformStack)` | — | **MISSING (P1)** |
| L59 | `Clear()` | `Clear` | **OK** |
| L60 | `SetTime(time: UsdTimeCode)` | `SetTime(time: float)` | **PARTIAL (P2)** |
| L61 | `GetTime()` → `UsdTimeCode` | — | **MISSING (P2)** |
| L63 | `Swap(other: XformCache)` | — | **MISSING (P2)** |

### 3.2 ID для коммитов

| ID | Суть |
|----|------|
| XC-GF | Матрицы **L2W / P2W** возвращать как **`Gf.Matrix4d`**, не flat list |
| XC-LCL | Добавить **`GetLocalTransformation`** → `(Gf.Matrix4d, bool)` |
| XC-REL | Добавить **`ComputeRelativeTransform(prim, ancestor)`** |
| XC-TIME | **`GetTime` / `SetTime`** — `Usd.TimeCode` в дополнение или вместо `float` |
| XC-SWAP | Добавить **`Swap`** |

### 3.3 Старые ID (обобщение)

- **XC1** заменён таблицей §3.1; доп. методы только из **`.h`**, не из wrap — в отдельную задачу «расширить Python как в C++ API», не как в pxr.
- **XC2** = **XC-GF** (тип матрицы).

---

## 4. `UsdGeom.BasisCurves` — построчная сверка с `wrapBasisCurves.cpp`

**Источник:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapBasisCurves.cpp` (класс `BasisCurves`, строки ~67–167; кастомный блок L142–168).  
**pxr:** `crates/usd-pyo3/src/geom.rs` (`PyBasisCurves`); логика в `crates/usd/usd-geom/src/basis_curves.rs` (`BasisCurves`).

В C++ `class_<UsdGeomBasisCurves, bases<UsdGeomCurves>>` — в Python **BasisCurves наследует Curves** (методы `GetCurveVertexCountsAttr`, `ComputeExtent`, цепочка к `PointBased` / `Gprim` / `Xformable` и т.д.). В PyO3 **`PyBasisCurves` не подкласс `PyCurves`**, но **те же имена методов** делегируются в `self.0.curves()` / static `Curves::compute_extent` (2026-04). **Остаётся:** нет настоящего `issubclass(BasisCurves, Curves)` (см. также §11 XF3).

### 4.1 Конструкторы

| wrap (строки) | C++ / pxr | pxr | Статус |
|---------------|-----------|--------|--------|
| L75 | `__init__(prim: UsdPrim)` | `BasisCurves(prim)` через `extract_prim` | **OK** по сценарию «из примитива» |
| L76 | `__init__(schemaObj: UsdSchemaBase)` (из `UsdGeomCurves` и т.п.) | — | **MISSING (P2)** — нет второго `#[new]` |

### 4.2 Статические методы

| wrap | C++ / pxr | pxr | Статус |
|------|-------------|--------|--------|
| L79–80 | `Get(stage, path)` | `Get` | **OK** (`path`: `str` \| `Sdf.Path`) |
| L82–83 | `Define(stage, path)` | `Define` | **OK** |
| L85–89 | `GetSchemaAttributeNames(includeInherited=True)` → `list` | `GetSchemaAttributeNames` @staticmethod | **OK** |
| L91–93 | `_GetStaticTfType()` | — | **MISSING (P2)** — как и у других схем без Tf-обёртки |

### 4.3 Атрибуты Type / Basis / Wrap

| wrap | Метод | pxr | Статус |
|------|-------|--------|--------|
| L98–103 | `GetTypeAttr`, `CreateTypeAttr(defaultValue=…, writeSparsely=…)` | `CreateTypeAttr(default_value=…, write_sparsely=…)` | **OK** (kwargs snake_case PyO3) |
| L105–110 | `GetBasisAttr`, `CreateBasisAttr(...)` | то же | **OK** |
| L112–117 | `GetWrapAttr`, `CreateWrapAttr(...)` | то же | **OK** |

### 4.4 Вспомогательные методы (кастомный блок WRAP_CUSTOM, L157–167)

Реализация в **`usd-geom`**; экспорт в **`geom.rs`** `PyBasisCurves`.

| wrap | C++ / pxr | pxr | Статус |
|------|-------------|--------|--------|
| L162 | `ComputeInterpolationForSize(n, time: UsdTimeCode)` → `Tf.Token` | `ComputeInterpolationForSize` → `str` | **OK** |
| L163 | `ComputeUniformDataSize(time)` | **OK** (`time` через `Usd.TimeCode` / `float` / default) |
| L164 | `ComputeVaryingDataSize(time)` | **OK** |
| L165 | `ComputeVertexDataSize(time)` | **OK** |
| L166 | `ComputeSegmentCounts(time)` → `list[int]` | **OK** |

### 4.5 Прочее

| Элемент | Статус |
|---------|--------|
| L119 | `__repr__` — у нас формат `UsdGeom.BasisCurves('path')` vs референс через `TfPyRepr(prim)` | **PARTIAL (P3)** — сравнение строк с тестами pxr |
| L95 | `!self` (булево «пустая схема») | Зависит от **наследования** и дублирования `__bool__` на подклассах |

### 4.6 ID для коммитов

| ID | Суть |
|----|------|
| BC-INH | **Наследование от `UsdGeom.Curves`** (и далее по цепочке схемы) для `BasisCurves` |
| BC-SCH | **`GetSchemaAttributeNames`** — **сделано** |
| BC-COMPUTE | **`ComputeInterpolationForSize`**, **`ComputeUniformDataSize`**, … — **сделано** |
| BC-CTOR | Второй конструктор из **schema** (`UsdSchemaBase`) |
| BC-ATTR-KW | **`Create*Attr`** — kwargs `defaultValue`, `writeSparsely` как в wrap |

### 4.7 `UsdGeom.Curves` — доп. методы из `wrapCurves.cpp` (кастомный блок L161–173)

**Источник:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapCurves.cpp` (после автоген. части: `GetCurveVertexCountsAttr`, `Create*`, `GetWidthsAttr`, …).

| wrap | C++ / pxr | pxr | Статус |
|------|------------|--------|--------|
| L163–165 | `GetWidthsInterpolation` / `SetWidthsInterpolation` | `geom.rs` `PyCurves` | **OK** |
| L167–172 | `@staticmethod ComputeExtent(points, widths)` | `PyCurves::compute_extent_curves` → `Option[list[Gf.Vec3f]]` (C++: пустой объект при провале) | **PARTIAL (P2)** — семантика «пусто» vs `None` |
| L170–171 | `GetCurveCount(timeCode=Default)` | `GetCurveCount(time=None)` → `usize` | **OK** |

*Детальная построчная таблица совпадает с §4 для общих полей Curves; эта подсекция фиксирует только **доп.** API сверх `BasisCurves`.*

### 4.8 `UsdGeom.Mesh` — `wrapMesh.cpp`

**Источник:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapMesh.cpp` (автоген.: все `Get*Attr` / `Create*Attr` с `defaultValue`, `writeSparsely`; кастом L270–300: `ValidateTopology`, `GetFaceCount`, `SHARPNESS_INFINITE`).

| Элемент | pxr / примечание |
|---------|-------------------|
| `Create*Attr` на топологии и subdiv | **OK** — kwargs `default_value`, `write_sparsely` (`geom.rs` `PyMesh`) |
| `CreateVelocitiesAttr` / `CreateNormalsAttr` / `CreateAccelerationsAttr` | **OK** — те же kwargs (PointBased) |
| `GetFaceCount(timeCode)` | **OK** — `time` через `tc_from_py_opt` (`Usd.TimeCode` / `float` / default) |
| `SHARPNESS_INFINITE` | **OK** — `#[classattr]` `SHARPNESS_INFINITE` (вместо только staticmethod `SharpnessInfinite`) |
| Второй ctor `schemaObj` | **MISSING (P2)** — как у других схем |
| `bases<UsdGeomPointBased>` | **PARTIAL** — методы PointBased на `Mesh` есть, `issubclass(Mesh, PointBased)` нет (PyO3) |

### 4.9 `UsdGeom.NurbsCurves`, `HermiteCurves`, `Sphere` (кратко)

| Файл wrap | pxr (2026-04) |
|-----------|----------------|
| `wrapNurbsCurves.cpp` | `GetPointWeightsAttr` / kwargs на Order·Knots·Ranges·PointWeights; делегаты **Curves** (`ComputeExtent`, vertex counts, widths, …); `GetSchemaAttributeNames` |
| `wrapHermiteCurves.cpp` | kwargs **`CreateTangentsAttr`**; делегаты **Curves**; `GetSchemaAttributeNames`. **MISSING:** вложенный тип **`PointAndTangentArrays`** (кастом L125–144) |
| `wrapSphere.cpp` | **`GetExtentAttr` / `CreateExtentAttr`**; kwargs radius/extent; `GetSchemaAttributeNames` |

---

## 5. `UsdGeom.Boundable` — построчная сверка с `wrapBoundable.cpp`

**Источник:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapBoundable.cpp` (класс `Boundable`, L57–168).  
**pxr:** `crates/usd-pyo3/src/geom.rs` (`PyBoundable`); логика — `crates/usd/usd-geom/src/boundable.rs`.

C++: `class_<UsdGeomBoundable, bases<UsdGeomXformable>>` — в Python **Boundable** наследует **Xformable**. У нас **`PyBoundable` не подкласс `PyXformable`** (методы `AddTranslateOp`, `ComputeLocalBound`, … недоступны на типе без отдельного моста — см. **§11 XF3**).

### 5.1 Конструкторы

| wrap | C++ / pxr | pxr | Статус |
|------|-------------|--------|--------|
| L61–62 | `__init__(prim)`, `__init__(schemaObj)` | только `Boundable(prim)` | **PARTIAL (P2)** — нет ctor из `schemaObj` |

### 5.2 Статические методы

| wrap | C++ / pxr | pxr | Статус |
|------|-------------|--------|--------|
| L65–66 | `Get(stage, path)` | `Get` | **OK** |
| L68–72 | `GetSchemaAttributeNames(includeInherited=True)` | `GetSchemaAttributeNames` | **OK** (2026-04) |
| L74–76 | `_GetStaticTfType()` | — | **MISSING (P2)** |

### 5.3 Extent

| wrap | Метод | pxr | Статус |
|------|-------|--------|--------|
| L81–86 | `GetExtentAttr`, `CreateExtentAttr(defaultValue=…, writeSparsely=…)` | `GetExtentAttr`, `CreateExtentAttr` без kwargs | **PARTIAL (P2)** |
| L159–161 | `ComputeExtent(time)` → `Vt.Vec3fArray` или **`None`** | `ComputeExtent` → `list[Gf.Vec3f]` или **`None`** | **OK** / **PARTIAL (P3)** — тип массива (Vt vs список `Gf.Vec3f`) |
| L162–166 | `ComputeExtentFromPlugins(boundable, time)` @staticmethod | `ComputeExtentFromPlugins` @staticmethod | **OK** |
| L164–166 | перегрузка `ComputeExtentFromPlugins(boundable, time, transform)` | тот же static, `transform=None` → `Gf.Matrix4d` | **OK** |

### 5.4 Прочее

| Элемент | Статус |
|---------|--------|
| L88 | `__repr__` | **PARTIAL (P3)** — формат пути vs `TfPyRepr(prim)` |
| L78 | `!self` | `__bool__` на обёртке — **OK** для самого `Boundable` |

### 5.5 ID

| ID | Суть |
|----|------|
| BO-INH | Наследование от **`UsdGeom.Xformable`** на уровне Python |
| BO-TF | **`_GetStaticTfType`** |
| BO-CTOR | Второй конструктор **`schemaObj`** |
| BO-ATTR-KW | **`CreateExtentAttr`** — kwargs как в wrap |

**Реализация:** `Boundable::compute_extent_from_plugins` в `usd-geom` — тонкая обёртка над integrated/plugin путём (как статический `ComputeExtentFromPlugins` в pxr).

---

## 6. `UsdGeom.Camera` — построчная сверка с `wrapCamera.cpp`

**Источник:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapCamera.cpp` (L172–369).  
**pxr:** `crates/usd-pyo3/src/geom.rs` (`PyCamera`); ядро — `crates/usd/usd-geom/src/camera.rs`. **`Gf.Camera`** — `crates/usd-pyo3/src/gf/geo.rs` (`pxr.Gf.Camera`).

C++: `bases<UsdGeomXformable>` — наследование от **Xformable** в Python; у нас **нет подкласса** `PyXformable` (см. **§11 XF3**).

### 6.1 Конструкторы и статика

| wrap | pxr | Статус |
|------|--------|--------|
| `Get`, `Define`, `GetSchemaAttributeNames` | есть | **OK** |
| `_GetStaticTfType` | — | **MISSING (P2)** |
| второй `__init__(schemaObj)` | — | **MISSING (P2)** |

### 6.2 Атрибуты (Get/Create*)

Все пары из wrap (projection, aperture, clipping, fStop, focus, stereo, shutter, **exposure / exposureIso / exposureTime / exposureFStop / exposureResponsivity**) — **OK**; `Create*Attr` без kwargs `defaultValue` / `writeSparsely` — **PARTIAL (P2)** как у других схем.

### 6.3 Кастомный блок (L356–366)

| Метод | pxr | Статус |
|-------|--------|--------|
| `GetCamera(time=Default)` → `Gf.Camera` | `GetCamera(time=None)` → `Gf.Camera` | **OK** |
| `SetFromCamera(camera, time=Default)` | `SetFromCamera(camera, time=None)` | **OK** |
| `ComputeLinearExposureScale(time=Default)` | `ComputeLinearExposureScale(time=None)` | **OK** |

**Было:** `GetCamera` возвращал **dict** с тремя полями — **исправлено** (паритет с pxr).

### 6.4 ID

| ID | Суть |
|----|------|
| CA-INH | Наследование от **`UsdGeom.Xformable`** |
| CA-TF | **`_GetStaticTfType`** |
| CA-CTOR | Второй конструктор **`schemaObj`** |
| CA-ATTR-KW | **`Create*Attr`** kwargs |

---

## 7. `UsdGeom.Capsule` / `Capsule_1` — сверка с `wrapCapsule.cpp` и `wrapCapsule_1.cpp`

**Источники:** `wrapCapsule.cpp` (L74–164, без кастомного блока), `wrapCapsule_1.cpp` (L81–177). Оба: `bases<UsdGeomGprim>`.  
**pxr:** `PyCapsule` / `PyCapsule1` в `geom.rs`; ядро — `usd-geom` `capsule.rs`.

### 7.1 Общее для обоих типов

| Элемент | Статус |
|---------|--------|
| `Get`, `Define` | **OK** |
| `GetSchemaAttributeNames` | **OK** — для **`Capsule_1`** в Rust добавлен `Capsule1::get_schema_attribute_names` (локальные имена: height, radiusTop, radiusBottom, axis, extent — без legacy `radius`) |
| `_GetStaticTfType`, второй ctor `schemaObj` | **MISSING (P2)** |
| Наследование от **`Gprim`** | **CAP-INH** — как у других схем, не подкласс `PyGprim` (см. **§11 XF3**) |
| `Create*Attr` kwargs | **PARTIAL (P2)** |

### 7.2 `Capsule` (`wrapCapsule.cpp`)

| Метод | pxr | Статус |
|-------|--------|--------|
| Height, Radius, Axis, **Extent** Get/Create | есть | **OK** (extent добавлен в биндинг) |

### 7.3 `Capsule_1` (`wrapCapsule_1.cpp`)

| Метод | pxr | Статус |
|-------|--------|--------|
| Height, RadiusTop, RadiusBottom, Axis, **Extent** Get/Create | через `as_capsule()` | **OK** |

### 7.4 ID

| ID | Суть |
|----|------|
| CAP-INH | Подкласс **`UsdGeom.Gprim`** в Python |
| CAP-TF | **`_GetStaticTfType`** |
| CAP-CTOR | Второй конструктор **`schemaObj`** |
| CAP-ATTR-KW | **`Create*Attr`** kwargs |

---

## 8. `UsdGeom.Cone` — построчная сверка с `wrapCone.cpp`

**Источник:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapCone.cpp` (L74–164, без кастомного блока). **C++:** `class_<UsdGeomCone, bases<UsdGeomGprim>>`.  
**pxr:** `crates/usd-pyo3/src/geom.rs` (`PyCone`); ядро — `crates/usd/usd-geom/src/cone.rs`.

| Элемент | pxr | Статус |
|---------|--------|--------|
| `Get`, `Define` | есть | **OK** |
| `GetSchemaAttributeNames` | есть | **OK** |
| `Get/Create` Height, Radius, Axis, **Extent** | есть | **OK** |
| `_GetStaticTfType`, второй ctor `schemaObj` | — | **MISSING (P2)** |
| Наследование **`Gprim`** | — | **CO-INH** — см. **§11 XF3** |
| `Create*Attr` kwargs | без kwargs | **PARTIAL (P2)** |

### ID для коммитов

| ID | Суть |
|----|------|
| CO-INH | Подкласс **`UsdGeom.Gprim`** в Python |
| CO-TF | **`_GetStaticTfType`** |
| CO-CTOR | Второй конструктор **`schemaObj`** |
| CO-ATTR-KW | **`Create*Attr`** kwargs |

---

## 9. `UsdGeom.ConstraintTarget` — построчная сверка с `wrapConstraintTarget.cpp`

**Источник:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapConstraintTarget.cpp` (L34–64).  
**pxr:** `crates/usd-pyo3/src/geom.rs` (`PyConstraintTarget`); ядро — `crates/usd/usd-geom/src/constraint_target.rs`.

| wrap | C++ / pxr | pxr | Статус |
|------|-------------|--------|--------|
| `__init__(attr: Usd.Attribute)` | `ConstraintTarget(attr)` | `ConstraintTarget(attr)` | **OK** |
| `GetAttr` | `Usd.Attribute` | `GetAttr` → `Usd.Attribute` | **OK** |
| `IsDefined` | | `IsDefined` | **OK** |
| `IsValid` | | `IsValid` | **OK** (через `is_valid_instance`) |
| `SetIdentifier` / `GetIdentifier` | `Tf.Token` | `str` | **PARTIAL (P2)** — тип идентификатора |
| `Get(time=Default)` | `Gf.Matrix4d` | `Gf.Matrix4d` \| `None` | **OK** |
| `Set(value, time=Default)` | | | **OK** |
| `GetConstraintAttrName` @staticmethod | | @staticmethod | **OK** |
| `ComputeInWorldSpace(time=Default)` | без `XformCache` в Python | `ComputeInWorldSpace(time=None)` без кэша | **OK** (как в C++ wrap) |
| неявное приведение к `Usd.Attribute` | `implicitly_convertible` | — | **MISSING (P2)** — при необходимости тестов |

### ID

| ID | Суть |
|----|------|
| CT-CONV | Неявное приведение **ConstraintTarget → Attribute** (как в pxr) |
| CT-ID-TYPE | **`GetIdentifier` / `SetIdentifier`** — `Tf.Token` vs `str` |

---

## 10. `UsdGeom.Cube` — построчная сверка с `wrapCube.cpp`

**Источник:** `usd-refs/OpenUSD/pxr/usd/usdGeom/wrapCube.cpp` (L60–135). **C++:** `bases<UsdGeomGprim>`.  
**pxr:** `crates/usd-pyo3/src/geom.rs` (`PyCube`); ядро — `crates/usd/usd-geom/src/cube.rs`.

| Элемент | pxr | Статус |
|---------|--------|--------|
| `Get`, `Define` | есть | **OK** |
| `GetSchemaAttributeNames` | есть | **OK** |
| `Get/Create` Size, **Extent** | есть | **OK** |
| `_GetStaticTfType`, второй ctor `schemaObj` | — | **MISSING (P2)** |
| наследование **Gprim** | — | **CU-INH** — см. **§11 XF3** |
| `Create*Attr` kwargs | без kwargs | **PARTIAL (P2)** |

### ID для коммитов

| ID | Суть |
|----|------|
| CU-INH | Подкласс **`UsdGeom.Gprim`** в Python |
| CU-TF | **`_GetStaticTfType`** |
| CU-CTOR | Второй конструктор **`schemaObj`** |
| CU-ATTR-KW | **`Create*Attr`** kwargs |

---

## 11. Геометрия: `Xformable` / `Imageable` / матрицы

| ID | Отклонение | Источник / примечание | Приоритет |
|----|------------|------------------------|-----------|
| XF1 | `Gf.Matrix4d.SetTranslate` / `SetTranslateOnly` / `SetScale`: кортеж **целых** `(2,2,2)` — нужна явная поддержка `(i64,i64,i64)` или общая нормализация в float (как в C++). | тесты `SetTranslate((2,)*3)` | P0 |
| XF2 | `UsdGeom.Xformable.GetLocalTransformation()` / цепочка xform: при расхождениях с C++ проверить **тип значения** в `xformOp:*` после Python `Set` (Vt / downcast в `get_op_transform_static`) и **время** запроса (`UsdTimeCode.Default` vs числовое). | отладка сессии | P0 |
| XF3 | Делегирование **Imageable/Xformable** на конкретных схемах — через `usd_geom_schema_with_xform!` + `GeomXformImg` (**OK** для Mesh/Camera/…). Отдельно: тип **`UsdGeom.Xformable`** (обёртка схемы без typed prim) **должен** включать API **Imageable** — в C++ `bases<UsdGeomImageable>`; добавлено в `geom.rs` (`GetVisibility*` … `ComputeWorldBound` / `ComputeLocalBound`, `GetOrderedPurposeTokens`). Остаётся: **нет** `issubclass(Mesh, Xformable)` (ограничение PyO3). | `geom.rs` `PyXformable` | P2 |
| XF4 | `usd_geom::XformQuery::from_xformable`: если используется путь с `AttributeQuery` vs без — влияет на чтение значений при default time (см. обсуждение в коде/коммите). | `usd-geom` `xformable.rs` | P0/P1 |

---

## 12. Модуль `Usd` (`usd.rs`)

| ID | Элемент | Отклонение | Приоритет |
|----|---------|------------|-----------|
| U1 | `Prim` / дети | `GetChildren` / фильтрация: **TODO** — предикаты не полные, «все дети». | P2 |
| U2 | `Attribute` | `GetPropertyStack` — **stub**, пустой список. | P2 |
| U3 | `Attribute` connections | Позиция в `AddConnection` / targets — **TODO** в коде. | P2 |
| U4 | `Relationship` | Позиция target — **TODO**. | P2 |
| U5 | `Stage` metadata | `SetMetadataByDictKey`, `ClearMetadataByDictKey`, `HasMetadataDictKey`, `HasAuthoredMetadataDictKey` — **возвращают false / не реализованы**. | P1 |
| U6 | `Stage` | `SetLoadRules` — **TODO**, нет привязки `StageLoadRules`. | P1 |
| U7 | `PrimDefinition` | **Stub** (реестр схем не полон). | P2 |
| U8 | Notices | `ObjectsChanged`, `StageContentsChanged`, `StageEditTargetChanged` — **stub**-уровень. | P2 |
| U9 | Schema info | Часть API поиска схем — **stub** (возвращает None/пусто). | P2 |
| U10 | `UsdClipsAPI` | Заглушка. | P2 |
| U11 | `ColorSpaceHashCache` | Заглушка для наследования в тестах. | P2 |
| U12 | Пути | Не везде `str | Sdf.Path` (частично исправлено в `UsdGeom`); **остальные модули** — добить по тестам. | P1 |

---

## 13. `Sdf` (`sdf.rs`)

| ID | Отклонение | Приоритет |
|----|------------|-----------|
| S1 | `VariableExpression` / AST — в основном **stub**-пространства имён для импортов тестов. | P2 |
| S2 | Часть API из `wrapSdf*.cpp` не покрыта (см. список в `PYTHON_API_PARITY.md`: FileFormat, CopySpec, ZipFileWriter, BatchNamespaceEdit, …). | P1 |

---

## 14. `Tf` (`tf.rs`)

| ID | Отклонение | Приоритет |
|----|------------|-----------|
| T1 | Notice / регистрация: **TODO** sender-scoped. | P2 |
| T2 | `SetOutputFile` — **no-op** stub. | P3 |
| T3 | Полный паритет с `wrapTf*.cpp` — большинство тестов **падают** (см. `TODO.md` по `base/tf`). | P1 |

---

## 15. `Vt` / `Gf` / прочее

| ID | Отклонение | Приоритет |
|----|------------|-----------|
| V1 | `py_to_value`: при появлении новых тестов — добить типы (**Vec3d[]**, **Quatf[]**, тензоры и т.д.). | P2 |
| V2 | `Gf.IsClose` — не для всех типов, как в референсе. | P2 |
| AR1 | `Ar` — часть поведения помечена как stub (контекст). | P2 |
| SH1 | `UsdShade` — `CanConnect` stub; шейдеры не все типы через Vt (комментарий в `shade.rs`). | P2 |
| SK1 | `UsdSkel` — `ComputeSkinnedPoints` stub (пустой результат / нет моста float). | P2 |
| PCP1 | `Pcp` — relocates: **stub**, пустой dict. | P2 |

---

## 16. Несоответствия «тип возврата» (частый класс отклонений)

Во многих местах pxr возвращает **плоские списки** (`Vec<f64>`) вместо **Gf-типов** (`Gf.Matrix4d`, `Gf.BBox3d`, `Gf.Vec3d`). Это:

- упрощает биндинг, но **ломает** тесты, использующие `.GetRange()`, операторы Gf, `repr`, `==` между Gf-объектами.

Систематически пройти: **Imageable.ComputeWorldBound/ComputeLocalBound**, **BBoxCache** (см. BB1), **XformCache** (XC2), любые **Compute*Transform**.

---

## 17. Сводка по pytest (из `TODO.md`, снимок 2026-04-07)

| Область | Passed | Failed | Errors | Примечание |
|---------|--------|--------|--------|------------|
| base/gf | 126 | 21 | 0 | Много мелких расхождений Gf |
| base/vt | 24 | 4 | 0 | |
| base/tf | 3 | ~41 | 0 | Tf сильно неполон |
| usd/sdf | 38 | 157 | 1 | |
| usd/usd | ~36 | ~430 | ~61 | Core |
| usd/usdGeom | 8 | 196 | 2 | |
| usd/usdSkel | 4 | 29 | 0 | |
| **Итого (подмножество)** | **283** | **887** | **61** | Полный прогон см. `TODO.md` |

После каждого крупного исправления: обновлять счётчики и **вычеркивать** пункты в этом файле или переносить в «исправлено» в `PYTHON_API_PARITY.md`.

---

## 18. Чек-лист сверки с референсом по файлам (не исчерпывающий)

Пока не все `wrap*.cpp` вычитаны вручную; приоритет — модули с высоким числом падений:

- [x] `pxr/usd/usdGeom/wrapBBoxCache.cpp` — построчно §2  
- [x] `pxr/usd/usdGeom/wrapXformCache.cpp` — построчно §3  
- [x] `pxr/usd/usdGeom/wrapBasisCurves.cpp` — построчно §4  
- [x] `pxr/usd/usdGeom/wrapBoundable.cpp` — построчно §5  
- [x] `pxr/usd/usdGeom/wrapCamera.cpp` — построчно §6  
- [x] `pxr/usd/usdGeom/wrapCapsule.cpp`, `wrapCapsule_1.cpp` — §7  
- [x] `pxr/usd/usdGeom/wrapCone.cpp` — §8  
- [x] `pxr/usd/usdGeom/wrapConstraintTarget.cpp` — §9  
- [x] `pxr/usd/usdGeom/wrapCube.cpp` — §10  
- [x] Пакет **статических** API схем (`GetSchemaAttributeNames`, корректный `includeInherited`, extent где есть в Rust) — см. **§20** (не заменяет построчную вычитку каждого wrap).  
- [x] `pxr/usd/usdGeom/wrapMesh.cpp` — §4.8 (автоген + кастом `SHARPNESS_INFINITE` / `GetFaceCount` / kwargs)  
- [x] `wrapNurbsCurves.cpp` / `wrapHermiteCurves.cpp` / `wrapSphere.cpp` — §4.9 (делегаты Curves, kwargs, Nurbs `PointWeights`, Hermite `PointAndTangentArrays` — **ещё нет**)  
- [ ] `pxr/usd/usdGeom/wrap*.cpp` — построчно **остальные** файлы (например `wrapNurbsPatch.cpp`, …).  
- [ ] `pxr/usd/usd/wrapStage.cpp`, `wrapAttribute.cpp`, `wrapPrim.cpp`  
- [ ] `pxr/base/tf/wrap*.cpp`  
- [ ] `pxr/usd/sdf/wrap*.cpp`  

---

## 19. Как не создавать дубликаты при исправлениях

1. Исправили поведение — добавьте строку в «Сделано» в `PYTHON_API_PARITY.md` и **удалите или пометьте** пункт здесь (или ведите ID в коммите).  
2. Новый класс отклонений — новый ID в соответствующей секции.  
3. Для спорных случаев укажите **точный** тест референса: `testUsdGeomPointInstancer.py::...`.

---

## 20. Пакет: `GetSchemaAttributeNames` / `include_inherited` / extent (2026-04-08)

**Контекст:** в OpenUSD Python схемы наследуют статические `GetSchemaAttributeNames(bool includeInherited)` и (для boundable-подобных) доступ к `extent`; часть обёрток изначально отсутствовала или игнорировала флаг наследования.

**Реализация pxr:** `crates/usd-pyo3/src/geom.rs` — делегирование в `usd_geom::*::get_schema_attribute_names(include_inherited)` и `#[pyo3(name = "GetSchemaAttributeNames", signature = (include_inherited = true))]` где применимо.

**Покрыто именами (класс Python → Rust):**  
Imageable, Xformable, Xform, Boundable, Scope, Gprim, Mesh, Sphere, Cube, Cone, Cylinder, Cylinder_1 (`Cylinder::get_schema_attribute_names`), Capsule, Capsule_1, Plane, PointBased, Points, Curves, BasisCurves, NurbsCurves, HermiteCurves, NurbsPatch, TetMesh, PointInstancer, Subset, Camera; **VisibilityAPI**, **ModelAPI**.

**Исправление флага:** у **Imageable**, **Xformable**, **Scope**, **Mesh** ранее `include_inherited` не пробрасывался — теперь передаётся в `usd_geom`.

**Extent (`GetExtentAttr` / `CreateExtentAttr`):** добавлены/используются для схем с boundable-путём в Rust, в т.ч. Sphere, Cylinder, Cylinder_1, Plane (и ранее Cube, Cone, Capsule, Capsule_1).

**Остаётся вне этого пакета:** неявное наследование **Gprim** в Python (см. §10 CU-INH), **XF3** (делегирование Imageable/Xformable на каждой конкретной схеме), возврат **Gf**-типов вместо плоских списков (§16), построчные таблицы для каждого `wrap*.cpp`.

| ID | Суть |
|----|------|
| SCH-STATIC | Пакет статических API и extent — **сделано** (этот §) |
| SCH-LINE | Построчная сверка **каждого** wrap-файла — **в процессе** (§18) |

---

## 21. `Sdr` — `shaderParserTestUtils` (нативная реализация, G4)

**Источник референса:** `usd-refs/OpenUSD/pxr/usd/sdr/shaderParserTestUtils.py` — модуль на **чистом Python**, импортируемый тестами парсеров OSL / Args и тестами `UsdShade` shader definitions.

**Реализация pxr:** `crates/usd-pyo3/src/sdr_shader_parser_test_utils.rs` — те же **публичные имена** (`IsNodeOSL`, `GetType`, `TestBasicProperties`, `TestShadingProperties`, `TestBasicNode`, `TestShaderSpecificNode`, `TestShaderPropertiesNode`), регистрация подмодуля `pxr.Sdr.shaderParserTestUtils` и запись в `sys.modules['pxr.Sdr.shaderParserTestUtils']`. Логика проверок идёт от **`usd_sdr::SdrShaderNode` / `SdrShaderProperty`** (через `PyShaderNode` / `PyShaderProperty::inner`).

**Намеренное отличие от Pixar (см. G4):** нет встроенного исходника `.py` в колесе и нет `compile`/`exec` внутри расширения — только один нативный биндинг + минимальный пакет `pxr`.

**`GetType(property)`:** возвращает `Tf.Type`, сопоставляя SDF-тип свойства через `ValueTypeName::cpp_type_name()` и `TfType::find_by_name` (эквивалент Python: `property.GetTypeAsSdfType().GetSdfType().type`).

**Проверка паритета:** после `maturin develop` прогнать тесты референса, которые импортируют `pxr.Sdr.shaderParserTestUtils`, например (пути от корня OpenUSD): `pxr/usd/plugin/sdrOsl/testenv/testOslParser.py`, `pxr/usd/usdShade/testenv/testUsdShadeShaderDef.py` (секции с `shaderParserTestUtils` / `TestShaderPropertiesNode*`). Локально: `crates/usd-pyo3/tests/usd/plugin/sdrOsl/testOslParser.py` и т.д. (нужен `OPENUSD_SRC_ROOT`).

**Сопутствующие правки порта (2026-04):**

| Тема | Суть |
|------|------|
| `Plug.Registry.FindTypeByName` | Если тип не из `plugInfo.json`, для имён классов парсеров SDR (`SdrOslParserPlugin`, …) вызывается `Tf.declare_by_name` — иначе тесты с `PXR_SDR_SKIP_PARSER_PLUGIN_DISCOVERY=1` получают `None`. См. `crates/usd-pyo3/src/plug.rs`. |
| `Sdr.NodeDiscoveryResult` | Как в OpenUSD Python: kwargs **`sourceCode`**, **`blindData`**, **`subIdentifier`** (camelCase), опциональные хвостовые аргументы позиционно. См. `sdr.rs`. |
| `Sdr.Registry.GetShaderNodeByIdentifier` | Второй аргумент **`type_priority`** опционален (`None` по умолчанию), как в референсе. |
| Реестр `usd_sdr` | Тип обнаружения **`oso`** обслуживает **`osl_parser::OslParserPlugin`** (байткод `.oso`); JSON **`.sdrOsl`** — только **`sdrosl_parser::SdrOslParserPlugin`** (раньше `oso` дублировался JSON-плагином, узел не парсился). `SetExtraParserPlugins` вставляет тот же **`OslParserPlugin`**, что и C++ OSL parser. |

**Остаётся:** `testOslParser.py` может ещё падать на тонком паритете метаданных / `Tf.Type` для строк и т.д. относительно эталонного OSL — это уже **`usd_sdr` + osl-rs**, не модуль `shaderParserTestUtils`.

---

*Документ сформирован для непрерывной работы «как танк»: ничего не пропускать; при обнаружении нового отклонения — дополнять таблицы.*
