# Рабочая память: OpenUSD Python-паритет (pxr)

**Назначение:** короткий контекст между сессиями; детали отклонений — в [`PYTHON_API_DEVIATIONS.md`](PYTHON_API_DEVIATIONS.md), инварианты — в [`PYTHON_API_PARITY.md`](PYTHON_API_PARITY.md).

## Процесс (зафиксировано)

- Короткие циклы: **один `wrap*.cpp` / одна схема** → строки в DEVIATIONS + **по возможности сразу код** (P0/P1), не откладывать всё на «после полного аудита».
- После блока: `cargo check -p usd-pyo3` (и pytest по затронутым тестам, если менялось поведение).

## Сейчас

| Поле | Значение |
|------|----------|
| Последний крупный блок | **§20 DEVIATIONS:** пакет `GetSchemaAttributeNames` + корректный `include_inherited` + extent там, где есть в `usd-geom`; журнал — `PYTHON_API_PARITY.md`. |
| Следующий приоритет (код) | **§11 XF3:** делегирование **Imageable** / **Xformable** на конкретных схемах (`Mesh`, `Sphere`, `Camera`, …) — как в C++ (или тесты `testUsdGeom*.py`). |
| Параллельно | Построчная вычитка **`wrapCurves.cpp`** → новая подсекция в DEVIATIONS (имена методов сверх `GetSchemaAttributeNames`). |
| PointBased | В Python: **`ComputePointsAtTime` / `ComputePointsAtTimes`** (`geom.rs`) — см. журнал PARITY. |

## Последнее обновление

- **2026-04-08:** Обновлены WORK / PARITY / DEVIATIONS §20 под пакет схем; добавлены `UsdGeom.PointBased` / `UsdGeom.Mesh` — `ComputePointsAtTime`, `ComputePointsAtTimes`.
- **2026-04-07:** `wrapCube.cpp` — код + §10; блок XF в DEVIATIONS — **§11**.
