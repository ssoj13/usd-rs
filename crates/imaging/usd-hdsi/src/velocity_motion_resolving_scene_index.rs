//! Velocity motion resolving scene index.
//!
//! Resolves velocity-based motion blur for points, instanceTranslations,
//! instanceRotations, and instanceScales.

use crate::tokens::VELOCITY_MOTION_RESOLVING_SCENE_INDEX_TOKENS;
use parking_lot::RwLock;
use std::ops::Deref;
use std::sync::Arc;
use usd_gf::{Quatf, Vec3f};
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdOverlayContainerDataSource, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource, HdSampledDataSource, HdSampledDataSourceTime,
    cast_to_container, hd_container_get,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::schema::{
    HdDependenciesSchema, HdDependencySchemaBuilder, HdPrimvarsSchema, HdSceneGlobalsSchema,
    PRIMVAR_VALUE,
};
use usd_hd::tokens;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;
use usd_vt::Value;

const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

const FALLBACK_TIME_CODES_PER_SECOND: f64 = 24.0;

fn primvar_affected_by_velocity(name: &TfToken) -> bool {
    name.as_str() == tokens::POINTS.as_str()
        || name.as_str() == tokens::INSTANCE_TRANSLATIONS.as_str()
        || name.as_str() == tokens::INSTANCE_ROTATIONS.as_str()
        || name.as_str() == tokens::INSTANCE_SCALES.as_str()
}

/// Applies angular velocities to quaternion rotations.
fn apply_angular_velocities(
    rotations: &[Quatf],
    velocities: &[Vec3f],
    scaled_time: f32,
) -> Vec<Quatf> {
    rotations
        .iter()
        .zip(velocity_iter(velocities, rotations.len()))
        .map(|(rot, vel)| {
            let vel_len = (vel.x * vel.x + vel.y * vel.y + vel.z * vel.z).sqrt();
            if vel_len < 1e-10 {
                return *rot;
            }
            let angle_rad = scaled_time * vel_len * DEG_TO_RAD; // USD angularVelocities in deg/s
            let axis = vel.normalized();
            let delta = Quatf::from_axis_angle(axis, angle_rad);
            *rot * delta
        })
        .collect()
}

fn velocity_iter(velocities: &[Vec3f], len: usize) -> impl Iterator<Item = Vec3f> + '_ {
    let last = if velocities.is_empty() {
        Vec3f::new(0.0, 0.0, 0.0)
    } else {
        velocities[velocities.len() - 1]
    };
    (0..len).map(move |i| velocities.get(i).copied().unwrap_or(last))
}

/// Velocity-resolving value data source.
struct VelocityValueDataSource {
    name: TfToken,
    source: HdDataSourceBaseHandle,
    prim_path: SdfPath,
    prim_source: HdContainerDataSourceHandle,
    input_scene: HdSceneIndexHandle,
}

impl VelocityValueDataSource {
    fn get_time_codes_per_second(&self) -> f64 {
        {
            let guard = self.input_scene.read();
            let sg = HdSceneGlobalsSchema::get_from_scene_index(guard.deref());
            if let Some(tcps_ds) = sg.get_time_codes_per_second() {
                return tcps_ds.get_typed_value(0.0);
            }
        }
        FALLBACK_TIME_CODES_PER_SECOND
    }

    fn get_mode(&self) -> TfToken {
        let vm_tokens = &*VELOCITY_MOTION_RESOLVING_SCENE_INDEX_TOKENS;
        // Velocity motion mode is at prim level: __velocityMotionMode
        let loc = HdDataSourceLocator::from_token(vm_tokens.velocity_motion_mode.clone());
        if let Some(ds) = hd_container_get(self.prim_source.clone(), &loc) {
            if let Some(sampled) = ds.as_ref().as_sampled() {
                let val = sampled.get_value(0.0);
                if let Some(t) = val.get::<TfToken>() {
                    let t_str = t.as_str();
                    if t_str == vm_tokens.disable.as_str()
                        || t_str == vm_tokens.enable.as_str()
                        || t_str == vm_tokens.ignore.as_str()
                        || t_str == vm_tokens.no_acceleration.as_str()
                    {
                        return TfToken::new(t_str);
                    }
                }
            }
        }
        vm_tokens.enable.clone()
    }

    fn velocity_motion_valid(
        &self,
        src_value: &mut Option<Value>,
        velocities: &mut Option<Vec<Vec3f>>,
        out_sample_time: &mut Option<f32>,
    ) -> bool {
        let velocities_token = if self.name == *tokens::INSTANCE_ROTATIONS {
            tokens::ANGULAR_VELOCITIES.clone()
        } else {
            tokens::VELOCITIES.clone()
        };
        let velocities_locator = HdPrimvarsSchema::get_default_locator()
            .append(&velocities_token)
            .append(&PRIMVAR_VALUE);

        let vel_ds = hd_container_get(self.prim_source.clone(), &velocities_locator);
        let vel_sampled = vel_ds.as_ref().and_then(|d| d.as_ref().as_sampled());
        let Some(vs) = vel_sampled else {
            return false;
        };

        let source_sampled = self.source.as_ref().as_sampled();
        let Some(ss) = source_sampled else {
            return false;
        };

        let mut src_times = Vec::new();
        let mut vel_times = Vec::new();
        ss.get_contributing_sample_times(0.0, 0.0, &mut src_times);
        vs.get_contributing_sample_times(0.0, 0.0, &mut vel_times);
        if src_times.is_empty() {
            src_times.push(0.0);
        }
        if vel_times.is_empty() {
            vel_times.push(0.0);
        }
        if src_times[0] != vel_times[0] {
            return false;
        }
        let sample_time = src_times[0];
        let vel_val = vs.get_value(sample_time);
        let src_val = ss.get_value(sample_time);

        let Some(vel_arr) = vel_val.get::<Vec<Vec3f>>() else {
            return false;
        };
        let src_len = value_array_len(&src_val);
        if src_len > vel_arr.len() {
            return false;
        }
        if self.name == *tokens::INSTANCE_ROTATIONS {
            if !src_val.is::<Vec<Quatf>>() && !src_val.is::<Vec<usd_gf::Quath>>() {
                return false;
            }
        } else if !src_val.is::<Vec<Vec3f>>() {
            return false;
        }

        *src_value = Some(src_val);
        *velocities = Some(vel_arr.clone());
        *out_sample_time = Some(sample_time);
        true
    }

    fn get_value_impl(&self, shutter_offset: f32) -> Value {
        let vm_tokens = &*VELOCITY_MOTION_RESOLVING_SCENE_INDEX_TOKENS;
        let mode = self.get_mode();
        if mode.as_str() == vm_tokens.ignore.as_str() {
            if let Some(s) = self.source.as_ref().as_sampled() {
                return s.get_value(shutter_offset);
            }
        }

        let mut src_value = None;
        let mut velocities = None;
        let mut sample_time = None;
        if !self.velocity_motion_valid(&mut src_value, &mut velocities, &mut sample_time) {
            if let Some(s) = self.source.as_ref().as_sampled() {
                return s.get_value(shutter_offset);
            }
            return Value::default();
        }

        let src = src_value.unwrap();
        let vel = velocities.unwrap();
        let st = sample_time.unwrap();

        if mode.as_str() == vm_tokens.disable.as_str() || self.name == *tokens::INSTANCE_SCALES {
            if let Some(s) = self.source.as_ref().as_sampled() {
                return s.get_value(st);
            }
        }

        let tcps = self.get_time_codes_per_second() as f32;
        let scaled_time = (shutter_offset - st) / tcps;

        if self.name == *tokens::INSTANCE_ROTATIONS {
            if let Some(rots) = src.get::<Vec<Quatf>>() {
                let result = apply_angular_velocities(rots, &vel, scaled_time);
                return Value::from(result);
            }
            if let Some(rots) = src.get::<Vec<usd_gf::Quath>>() {
                let quatf_rots: Vec<Quatf> = rots
                    .iter()
                    .map(|q| {
                        let qd = usd_gf::Quatd::from(*q);
                        Quatf::new(
                            qd.real() as f32,
                            usd_gf::Vec3f::new(
                                qd.imaginary().x as f32,
                                qd.imaginary().y as f32,
                                qd.imaginary().z as f32,
                            ),
                        )
                    })
                    .collect();
                let result = apply_angular_velocities(&quatf_rots, &vel, scaled_time);
                return Value::from(result);
            }
        }

        if let Some(positions) = src.get::<Vec<Vec3f>>() {
            let mut result = Vec::with_capacity(positions.len());
            for (i, pos) in positions.iter().enumerate() {
                let v = vel.get(i).copied().unwrap_or_else(|| {
                    vel.last()
                        .copied()
                        .unwrap_or_else(|| Vec3f::new(0.0, 0.0, 0.0))
                });
                result.push(Vec3f::new(
                    pos.x + scaled_time * v.x,
                    pos.y + scaled_time * v.y,
                    pos.z + scaled_time * v.z,
                ));
            }
            return Value::from(result);
        }

        if let Some(s) = self.source.as_ref().as_sampled() {
            s.get_value(shutter_offset)
        } else {
            Value::default()
        }
    }
}

fn value_array_len(v: &Value) -> usize {
    if let Some(arr) = v.get::<Vec<Vec3f>>() {
        return arr.len();
    }
    if let Some(arr) = v.get::<Vec<Quatf>>() {
        return arr.len();
    }
    if let Some(arr) = v.get::<Vec<usd_gf::Quath>>() {
        return arr.len();
    }
    0
}

impl std::fmt::Debug for VelocityValueDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VelocityValueDataSource")
            .field("name", &self.name)
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

impl HdDataSourceBase for VelocityValueDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            name: self.name.clone(),
            source: self.source.clone(),
            prim_path: self.prim_path.clone(),
            prim_source: self.prim_source.clone(),
            input_scene: self.input_scene.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdSampledDataSource for VelocityValueDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        self.get_value_impl(shutter_offset)
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        let vm_tokens = &*VELOCITY_MOTION_RESOLVING_SCENE_INDEX_TOKENS;
        let mode = self.get_mode();
        if mode.as_str() == vm_tokens.ignore.as_str() {
            if let Some(s) = self.source.as_ref().as_sampled() {
                return s.get_contributing_sample_times(start_time, end_time, out_sample_times);
            }
        }

        let mut src_value = None;
        let mut velocities = None;
        let mut sample_time = None;
        if !self.velocity_motion_valid(&mut src_value, &mut velocities, &mut sample_time) {
            if let Some(s) = self.source.as_ref().as_sampled() {
                return s.get_contributing_sample_times(start_time, end_time, out_sample_times);
            }
            return false;
        }

        if mode.as_str() == vm_tokens.disable.as_str() || self.name == *tokens::INSTANCE_SCALES {
            out_sample_times.clear();
            return false;
        }

        out_sample_times.clear();
        out_sample_times.push(start_time);
        out_sample_times.push(end_time);
        true
    }
}

/// Primvar container wrapper that applies velocity to primvarValue when affected.
#[derive(Clone)]
struct PrimvarDataSource {
    name: TfToken,
    source: Option<HdContainerDataSourceHandle>,
    prim_path: SdfPath,
    prim_source: HdContainerDataSourceHandle,
    input_scene: HdSceneIndexHandle,
}

impl std::fmt::Debug for PrimvarDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimvarDataSource")
            .field("name", &self.name)
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

impl HdContainerDataSource for PrimvarDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.source
            .as_ref()
            .map(|s| s.get_names())
            .unwrap_or_default()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let source = self.source.as_ref()?;
        let ds = source.get(name)?;
        if name.as_str() == PRIMVAR_VALUE.as_str() {
            if ds.as_ref().as_sampled().is_some() {
                let velocity_ds = Arc::new(VelocityValueDataSource {
                    name: self.name.clone(),
                    source: ds,
                    prim_path: self.prim_path.clone(),
                    prim_source: self.prim_source.clone(),
                    input_scene: self.input_scene.clone(),
                });
                return Some(velocity_ds as HdDataSourceBaseHandle);
            }
        }
        Some(ds)
    }
}

impl HdDataSourceBase for PrimvarDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            name: self.name.clone(),
            source: self.source.clone(),
            prim_path: self.prim_path.clone(),
            prim_source: self.prim_source.clone(),
            input_scene: self.input_scene.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

/// Primvars container wrapper.
#[derive(Clone)]
struct PrimvarsDataSource {
    source: Option<HdContainerDataSourceHandle>,
    prim_path: SdfPath,
    prim_source: HdContainerDataSourceHandle,
    input_scene: HdSceneIndexHandle,
}

impl std::fmt::Debug for PrimvarsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimvarsDataSource")
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

impl HdContainerDataSource for PrimvarsDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.source
            .as_ref()
            .map(|s| s.get_names())
            .unwrap_or_default()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let source = self.source.as_ref()?;
        let ds = source.get(name)?;
        if primvar_affected_by_velocity(name) {
            let container = cast_to_container(&ds)?;
            let wrapped = Arc::new(PrimvarDataSource {
                name: name.clone(),
                source: Some(container),
                prim_path: self.prim_path.clone(),
                prim_source: self.prim_source.clone(),
                input_scene: self.input_scene.clone(),
            });
            return Some(wrapped as HdDataSourceBaseHandle);
        }
        Some(ds)
    }
}

impl HdDataSourceBase for PrimvarsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            source: self.source.clone(),
            prim_path: self.prim_path.clone(),
            prim_source: self.prim_source.clone(),
            input_scene: self.input_scene.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

/// Prim data source that overlays primvars and __dependencies.
#[derive(Clone)]
struct PrimDataSource {
    prim_path: SdfPath,
    prim_source: Option<HdContainerDataSourceHandle>,
    input_scene: HdSceneIndexHandle,
}

impl std::fmt::Debug for PrimDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimDataSource")
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

impl HdContainerDataSource for PrimDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        let mut names = self
            .prim_source
            .as_ref()
            .map(|s| s.get_names())
            .unwrap_or_default();
        let deps_token = (*HdDependenciesSchema::get_schema_token()).clone();
        if !names.iter().any(|n| n.as_str() == deps_token.as_str()) {
            names.push(deps_token);
        }
        names
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let prim_source = self.prim_source.as_ref()?;
        let ds = prim_source.get(name)?;

        if name.as_str() == HdDependenciesSchema::get_schema_token().as_str() {
            let dep = HdDependencySchemaBuilder::default()
                .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::new(
                    HdSceneGlobalsSchema::get_default_prim_path(),
                ))
                .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
                    HdSceneGlobalsSchema::get_time_codes_per_second_locator(),
                ))
                .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
                    HdDataSourceLocator::empty(),
                ))
                .build();
            let overlay = HdRetainedContainerDataSource::from_arrays(
                &[usd_tf::Token::new("prim_dep_globals_timeCodesPerSecond")],
                &[dep as HdDataSourceBaseHandle],
            );
            if let Some(cont) = cast_to_container(&ds) {
                return Some(
                    HdOverlayContainerDataSource::new_2(overlay, cont) as HdDataSourceBaseHandle
                );
            }
            return Some(overlay as HdDataSourceBaseHandle);
        }

        if name.as_str() == HdPrimvarsSchema::get_schema_token().as_str() {
            if let Some(cont) = cast_to_container(&ds) {
                let wrapped = Arc::new(PrimvarsDataSource {
                    source: Some(cont),
                    prim_path: self.prim_path.clone(),
                    prim_source: prim_source.clone(),
                    input_scene: self.input_scene.clone(),
                });
                return Some(wrapped as HdDataSourceBaseHandle);
            }
        }

        Some(ds)
    }
}

impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            prim_path: self.prim_path.clone(),
            prim_source: self.prim_source.clone(),
            input_scene: self.input_scene.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

fn prim_type_supports_velocity_motion(prim_type: &TfToken) -> bool {
    let s = prim_type.as_str();
    s == tokens::RPRIM_POINTS.as_str()
        || s == tokens::RPRIM_BASIS_CURVES.as_str()
        || s == tokens::RPRIM_NURBS_CURVES.as_str()
        || s == tokens::RPRIM_NURBS_PATCH.as_str()
        || s == tokens::RPRIM_TET_MESH.as_str()
        || s == tokens::RPRIM_MESH.as_str()
        || s == tokens::INSTANCER.as_str()
}

/// Scene index that resolves velocity-based motion blur.
pub struct HdsiVelocityMotionResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdsiVelocityMotionResolvingSceneIndex {
    /// Creates a new velocity motion resolving scene index.
    pub fn new(
        input_scene: HdSceneIndexHandle,
        _input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene
                .read()
                .add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    /// Returns true if the prim type supports velocity motion.
    pub fn prim_type_supports_velocity_motion(prim_type: &TfToken) -> bool {
        prim_type_supports_velocity_motion(prim_type)
    }
}

impl HdSceneIndexBase for HdsiVelocityMotionResolvingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };

        if prim_type_supports_velocity_motion(&prim.prim_type) {
            if let Some(container) = &prim.data_source {
                let input = self.base.get_input_scene().unwrap().clone();
                prim.data_source = Some(Arc::new(PrimDataSource {
                    prim_path: prim_path.clone(),
                    prim_source: Some(container.clone()),
                    input_scene: input,
                }) as HdContainerDataSourceHandle);
            }
        }
        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiVelocityMotionResolvingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiVelocityMotionResolvingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if entries.len() >= 1000 {
            let first = entries
                .first()
                .map(|e| e.prim_path.to_string())
                .unwrap_or_default();
            eprintln!(
                "[velocity_motion] on_prims_dirtied in={} sender={} first={}",
                entries.len(),
                sender.get_display_name(),
                first,
            );
        }
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
