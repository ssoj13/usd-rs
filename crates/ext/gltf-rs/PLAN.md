# gltf-rs Improvement Plan

План доработок после копирования gltf в gltf-rs.

**Статус: выполнено**

---

## 1. Идентификация пакета и workspace

### 1.1 Переименование
- [x] **package name**: `gltf` → `gltf-rs` в корневом `Cargo.toml`
- [x] **repository/homepage**: `https://github.com/gltf-rs/gltf` → репозиторий usd-rs/vfx-rs
- [x] **documentation**: `https://docs.rs/gltf` → `https://docs.rs/gltf-rs` (или внутренняя ссылка)
- [x] **badges**: удалить `travis-ci` (устаревший), при необходимости добавить актуальные

### 1.2 Workspace
- [x] **usd-rs workspace**: добавить `gltf-json` и `gltf-derive` в `workspace.members` родительского `Cargo.toml`, чтобы CI/форматирование охватывали все crates
- [ ] Либо оставить вложенный workspace — тогда `cargo test -p gltf-rs` в корне может не подхватить sub-crates

---

## 2. Выравнивание с usd-rs

### 2.1 Edition и Rust
- [x] **edition**: `2021` → `2024` (как в workspace)
- [x] **rust-version**: `1.61` → `1.85` (или из `workspace.package`)
- [ ] Применить во всех трёх crates: gltf-rs, gltf-json, gltf-derive

### 2.2 version.workspace
- [x] Использовать `version.workspace = true` (и др. `*.workspace = true`) в `Cargo.toml`, где это уместно
- [ ] Версии под-crates выровнять с основным пакетом (0.1.0 или как в workspace)

### 2.3 workspace.dependencies
- [ ] Подключить общие зависимости через `workspace = true`:
  - `serde`, `serde_json`, `approx` — если есть в workspace
- [ ] `bytemuck` — проверить версию в workspace (если используется)

---

## 3. Зависимости

### 3.1 serde_derive
- [x] **gltf-json**: убрать `serde_derive = "1.0"`, использовать `serde` с `features = ["derive"]`
- [ ] Заменить `#[serde_derive::Deserialize]` → `#[serde::Deserialize]` и т.п. (если используется)

### 3.2 byteorder
- [ ] Рассмотреть переход на `std::Read`/`byte order` из Rust 1.85 или оставить `byteorder` для совместимости
- [ ] Низкий приоритет — `byteorder` стабилен

### 3.3 Устаревшие версии
- [ ] `image 0.25` → проверить актуальность, при необходимости обновить
- [ ] `base64 0.13`, `urlencoding 2.1` — проверить совместимость

---

## 4. Безопасность и паники

### 4.1 Критичные unwrap()
- [x] **import.rs:52** — `urlencoding::decode(uri).unwrap()`  
  Обработать `Err`: `decode(uri).map_err(Error::UnsupportedScheme)` или добавить `Error::InvalidUri`
- [ ] **import.rs:62** — `base.unwrap()` после `base.is_some()`: заменить на `if let Some(base) = base`

### 4.2 unreachable!()
- [x] **mesh/mod.rs** (read_colors, read_indices, read_joints, read_tex_coords, read_weights) — 5× `unreachable!()`  
  Добавить `_ => return None` или `Err` вместо паники для невалидных accessor-типов (если используется без валидации)
- [ ] **animation/util/mod.rs** — 2× `unreachable!()`
- [ ] **accessor/sparse.rs** — 1× `unreachable!()`
- [ ] Вариант: заменить на `debug_assert!(matches!(..., ...), "unexpected accessor type")` чтобы падать только в debug

### 4.3 unwrap() после валидации
- [ ] ~50 `unwrap()` в scene, mesh, material, texture и т.д.  
  Оставить как есть при условии, что все пути проходят через валидацию.  
  Документировать: «при использовании `*_without_validation` возможны паники».
- [ ] Альтернатива: постепенно заменить на `Result`/`Option` с явными ошибками для чувствительных API

---

## 5. Опечатки и косметика

- [x] **binary.rs**: `exceeeds` → `exceeds`, `occured` → `occurred`
- [x] **validation.rs** (gltf-json): `occured` → `occurred`

---

## 6. Тесты

### 6.1 import_sample_models
- [x] Тест требует `glTF-Sample-Assets/Models` — CI клонирует репозиторий
- [ ] Добавить `#[ignore]` или `#[cfg(has_sample_assets)]` для локального запуска без клона
- [ ] Документировать в README:  
  `cargo test --test import_sample_models` требует `git clone ... glTF-Sample-Assets`

### 6.2 box_sparse.glb
- [x] Проверить наличие `tests/box_sparse.glb`  
  Сейчас есть только `box_sparse.gltf`; тесты `roundtrip_binary_gltf` и `import_sample_models` ожидают `.glb`
- [ ] Сгенерировать `.glb` из `.gltf` или добавить готовый файл

### 6.3 Пути в тестах
- [ ] `tests/` выполняется с `cargo test` из корня crate — пути `tests/box_sparse.gltf` должны быть корректны
- [ ] При вложенном workspace — убедиться, что `cargo test -p gltf` запускается из `gltf-rs/`

---

## 7. CI/CD

### 7.1 GitHub Actions
- [x] `.github/workflows/test.yml` — пути к репозиторию glTF-Sample-Assets  
  Проверить, что `path: glTF-Sample-Assets` корректен относительно `crates/gltf-rs`
- [ ] `cargo test --all` — при вложенном workspace запуск из `gltf-rs/` должен покрывать gltf, gltf-json, gltf-derive
- [ ] `actions/checkout@v2` → обновить до v4
- [ ] Интеграция с CI родительского usd-rs, если есть

### 7.2 clippy.toml
- [ ] Проверить, что `clippy.toml` актуален и не конфликтует с настройками workspace

---

## 8. Документация

- [x] **README.md**: обновить примеры с `gltf` → `gltf-rs`
- [ ] Добавить раздел «Интеграция с usd-rs» при необходимости
- [ ] Указать MIT OR Apache-2.0, авторство gltf-rs

---

## 9. Опционально (низкий приоритет)

- [ ] **include** в Cargo.toml: убрать лишние файлы из пакета при публикации
- [ ] **guess_mime_type**: проверить feature на актуальность
- [ ] Реэкспорт: `pub use gltf::*` из gltf-rs для совместимости с `extern crate gltf` в старом коде

---

## Приоритеты

| Приоритет | Задачи |
|-----------|--------|
| **P0** | 1.1 (package name), 2.1 (edition), 6.2 (box_sparse.glb) |
| **P1** | 2.2, 2.3, 3.1 (serde), 4.1 (urlencoding), 7.1 (CI paths) |
| **P2** | 4.2 (unreachable), 5, 6.1, 8 |
| **P3** | 3.2, 9 |
