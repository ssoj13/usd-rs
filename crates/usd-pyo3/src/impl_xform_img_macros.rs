// Included from `geom.rs` — UsdGeom Xformable + Imageable (pxr parity).
//
// PyO3 0.28+ forbids macro invocations inside `#[pymethods]` impl blocks, and only one
// `#[pymethods] impl` per type is allowed. Schema-specific methods are passed as `tt` and
// merged with shared Xformable/Imageable methods at the macro call site.

macro_rules! usd_geom_schema_with_xform {
    ($py:ty, yes_get_path, { $($body:tt)* }) => {
        #[pymethods]
        impl $py {
            $($body)*
            #[pyo3(name = "GetPath")]
            pub fn get_path(&self) -> crate::sdf::PyPath {
                crate::sdf::PyPath::from_path(self.0.prim().path().clone())
            }
            #[pyo3(name = "GetXformOpOrderAttr")]
            pub fn get_xform_op_order_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).get_xform_op_order_attr(),
                )
            }
            #[pyo3(name = "CreateXformOpOrderAttr")]
            pub fn create_xform_op_order_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .create_xform_op_order_attr(),
                )
            }
            #[pyo3(name = "AddTranslateOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
            pub fn add_translate_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::Translate,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddRotateXYZOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_rotate_xyz_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::RotateXYZ,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddScaleOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_scale_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::Scale,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddTransformOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
            pub fn add_transform_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::Transform,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddOrientOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
            pub fn add_orient_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::Orient,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddRotateXOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_rotate_x_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::RotateX,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddRotateYOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_rotate_y_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::RotateY,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddRotateZOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_rotate_z_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::RotateZ,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddXformOp", signature = (op_type, precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
            pub fn add_xform_op(
                &self,
                op_type: &str,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let op_t = parse_xform_op_type(op_type)?;
                let prec = parse_xform_precision(precision)?;
                let tok = suffix.map(Token::new);
                Ok(PyXformOp(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                        op_t,
                        prec,
                        tok.as_ref(),
                        is_inverse_op,
                    ),
                ))
            }
            #[pyo3(name = "GetOrderedXformOps")]
            pub fn get_ordered_xform_ops(&self) -> Vec<PyXformOp> {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .get_ordered_xform_ops()
                    .into_iter()
                    .map(PyXformOp)
                    .collect()
            }
            /// C++ `UsdGeomXformable::GetLocalTransformation(UsdTimeCode) -> GfMatrix4d`.
            #[pyo3(name = "GetLocalTransformation", signature = (time=None))]
            pub fn get_local_transformation(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mat = crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .get_local_transformation(t);
                Ok(crate::gf::matrix::PyMatrix4d(mat))
            }
            #[pyo3(name = "TransformMightBeTimeVarying")]
            pub fn transform_might_be_time_varying(&self) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).transform_might_be_time_varying()
            }
            #[pyo3(name = "MakeMatrixXform")]
            pub fn make_matrix_xform(&self) -> PyXformOp {
                PyXformOp(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).make_matrix_xform(),
                )
            }
            #[pyo3(name = "ClearXformOpOrder")]
            pub fn clear_xform_op_order(&self) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).clear_xform_op_order()
            }
            #[pyo3(name = "GetResetXformStack")]
            pub fn get_reset_xform_stack(&self) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).get_reset_xform_stack()
            }
            #[pyo3(name = "SetResetXformStack")]
            pub fn set_reset_xform_stack(&self, reset: bool) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).set_reset_xform_stack(reset)
            }
            #[pyo3(name = "GetVisibilityAttr")]
            pub fn get_visibility_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .imageable()
                        .get_visibility_attr(),
                )
            }
            #[pyo3(name = "CreateVisibilityAttr")]
            pub fn create_visibility_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .imageable()
                        .create_visibility_attr(),
                )
            }
            #[pyo3(name = "GetPurposeAttr")]
            pub fn get_purpose_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .imageable()
                        .get_purpose_attr(),
                )
            }
            #[pyo3(name = "CreatePurposeAttr")]
            pub fn create_purpose_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .imageable()
                        .create_purpose_attr(),
                )
            }
            #[pyo3(name = "ComputeVisibility", signature = (time=None))]
            pub fn compute_visibility(&self, time: Option<f64>) -> String {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .imageable()
                    .compute_visibility(tc(time))
                    .as_str()
                    .to_owned()
            }
            #[pyo3(name = "ComputePurpose")]
            pub fn compute_purpose(&self) -> String {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .imageable()
                    .compute_purpose()
                    .as_str()
                    .to_owned()
            }
            #[pyo3(name = "MakeVisible", signature = (time=None))]
            pub fn make_visible(&self, time: Option<f64>) {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .imageable()
                    .make_visible(tc(time));
            }
            #[pyo3(name = "MakeInvisible", signature = (time=None))]
            pub fn make_invisible(&self, time: Option<f64>) {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .imageable()
                    .make_invisible(tc(time));
            }
            #[pyo3(name = "ComputeWorldBound")]
            pub fn compute_world_bound(&self, time: f64, purpose: &str) -> Vec<f64> {
                let mut cache = BBoxCache::new(
                    TimeCode::new(time),
                    vec![Token::new(purpose)],
                    false,
                    false,
                );
                bbox_to_flat(
                    &cache.compute_world_bound(
                        crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).imageable().prim(),
                    ),
                )
            }
            #[pyo3(name = "ComputeLocalBound")]
            pub fn compute_local_bound(&self, time: f64, purpose: &str) -> Vec<f64> {
                let mut cache = BBoxCache::new(
                    TimeCode::new(time),
                    vec![Token::new(purpose)],
                    false,
                    false,
                );
                bbox_to_flat(
                    &cache.compute_local_bound(
                        crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).imageable().prim(),
                    ),
                )
            }
            #[pyo3(name = "ComputeLocalToWorldTransform", signature = (time=None))]
            pub fn compute_local_to_world_transform(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mut cache = XformCache::new(t);
                let m = cache.get_local_to_world_transform(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).imageable().prim(),
                );
                Ok(crate::gf::matrix::PyMatrix4d(m))
            }
            #[pyo3(name = "ComputeParentToWorldTransform", signature = (time=None))]
            pub fn compute_parent_to_world_transform(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mut cache = XformCache::new(t);
                let m = cache.get_parent_to_world_transform(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).imageable().prim(),
                );
                Ok(crate::gf::matrix::PyMatrix4d(m))
            }
            #[staticmethod]
            #[pyo3(name = "GetOrderedPurposeTokens")]
            pub fn get_ordered_purpose_tokens() -> Vec<String> {
                Imageable::get_ordered_purpose_tokens()
                    .iter()
                    .map(|t| t.as_str().to_owned())
                    .collect()
            }
            #[pyo3(name = "IsValid")]
            pub fn is_valid_schema(&self) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).is_valid()
            }
        }
    };
    ($py:ty, no_get_path, { $($body:tt)* }) => {
        #[pymethods]
        impl $py {
            $($body)*
            #[pyo3(name = "GetXformOpOrderAttr")]
            pub fn get_xform_op_order_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).get_xform_op_order_attr(),
                )
            }
            #[pyo3(name = "CreateXformOpOrderAttr")]
            pub fn create_xform_op_order_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .create_xform_op_order_attr(),
                )
            }
            #[pyo3(name = "AddTranslateOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
            pub fn add_translate_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::Translate,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddRotateXYZOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_rotate_xyz_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::RotateXYZ,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddScaleOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_scale_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::Scale,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddTransformOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
            pub fn add_transform_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::Transform,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddOrientOp", signature = (precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
            pub fn add_orient_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Double);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::Orient,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddRotateXOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_rotate_x_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::RotateX,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddRotateYOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_rotate_y_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::RotateY,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddRotateZOp", signature = (precision="PrecisionFloat", suffix=None, is_inverse_op=false))]
            pub fn add_rotate_z_op(
                &self,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let prec = parse_xform_precision(precision).unwrap_or(XformOpPrecision::Float);
                let tok = suffix.map(Token::new);
                check_xform_op(crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                    XformOpType::RotateZ,
                    prec,
                    tok.as_ref(),
                    is_inverse_op,
                ))
            }
            #[pyo3(name = "AddXformOp", signature = (op_type, precision="PrecisionDouble", suffix=None, is_inverse_op=false))]
            pub fn add_xform_op(
                &self,
                op_type: &str,
                precision: &str,
                suffix: Option<&str>,
                is_inverse_op: bool,
            ) -> PyResult<PyXformOp> {
                let op_t = parse_xform_op_type(op_type)?;
                let prec = parse_xform_precision(precision)?;
                let tok = suffix.map(Token::new);
                Ok(PyXformOp(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).add_xform_op(
                        op_t,
                        prec,
                        tok.as_ref(),
                        is_inverse_op,
                    ),
                ))
            }
            #[pyo3(name = "GetOrderedXformOps")]
            pub fn get_ordered_xform_ops(&self) -> Vec<PyXformOp> {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .get_ordered_xform_ops()
                    .into_iter()
                    .map(PyXformOp)
                    .collect()
            }
            /// C++ `UsdGeomXformable::GetLocalTransformation(UsdTimeCode) -> GfMatrix4d`.
            #[pyo3(name = "GetLocalTransformation", signature = (time=None))]
            pub fn get_local_transformation(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mat = crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .get_local_transformation(t);
                Ok(crate::gf::matrix::PyMatrix4d(mat))
            }
            #[pyo3(name = "TransformMightBeTimeVarying")]
            pub fn transform_might_be_time_varying(&self) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).transform_might_be_time_varying()
            }
            #[pyo3(name = "MakeMatrixXform")]
            pub fn make_matrix_xform(&self) -> PyXformOp {
                PyXformOp(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).make_matrix_xform(),
                )
            }
            #[pyo3(name = "ClearXformOpOrder")]
            pub fn clear_xform_op_order(&self) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).clear_xform_op_order()
            }
            #[pyo3(name = "GetResetXformStack")]
            pub fn get_reset_xform_stack(&self) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).get_reset_xform_stack()
            }
            #[pyo3(name = "SetResetXformStack")]
            pub fn set_reset_xform_stack(&self, reset: bool) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).set_reset_xform_stack(reset)
            }
            #[pyo3(name = "GetVisibilityAttr")]
            pub fn get_visibility_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .imageable()
                        .get_visibility_attr(),
                )
            }
            #[pyo3(name = "CreateVisibilityAttr")]
            pub fn create_visibility_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .imageable()
                        .create_visibility_attr(),
                )
            }
            #[pyo3(name = "GetPurposeAttr")]
            pub fn get_purpose_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .imageable()
                        .get_purpose_attr(),
                )
            }
            #[pyo3(name = "CreatePurposeAttr")]
            pub fn create_purpose_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                        .imageable()
                        .create_purpose_attr(),
                )
            }
            #[pyo3(name = "ComputeVisibility", signature = (time=None))]
            pub fn compute_visibility(&self, time: Option<f64>) -> String {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .imageable()
                    .compute_visibility(tc(time))
                    .as_str()
                    .to_owned()
            }
            #[pyo3(name = "ComputePurpose")]
            pub fn compute_purpose(&self) -> String {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .imageable()
                    .compute_purpose()
                    .as_str()
                    .to_owned()
            }
            #[pyo3(name = "MakeVisible", signature = (time=None))]
            pub fn make_visible(&self, time: Option<f64>) {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .imageable()
                    .make_visible(tc(time));
            }
            #[pyo3(name = "MakeInvisible", signature = (time=None))]
            pub fn make_invisible(&self, time: Option<f64>) {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0)
                    .imageable()
                    .make_invisible(tc(time));
            }
            #[pyo3(name = "ComputeWorldBound")]
            pub fn compute_world_bound(&self, time: f64, purpose: &str) -> Vec<f64> {
                let mut cache = BBoxCache::new(
                    TimeCode::new(time),
                    vec![Token::new(purpose)],
                    false,
                    false,
                );
                bbox_to_flat(
                    &cache.compute_world_bound(
                        crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).imageable().prim(),
                    ),
                )
            }
            #[pyo3(name = "ComputeLocalBound")]
            pub fn compute_local_bound(&self, time: f64, purpose: &str) -> Vec<f64> {
                let mut cache = BBoxCache::new(
                    TimeCode::new(time),
                    vec![Token::new(purpose)],
                    false,
                    false,
                );
                bbox_to_flat(
                    &cache.compute_local_bound(
                        crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).imageable().prim(),
                    ),
                )
            }
            #[pyo3(name = "ComputeLocalToWorldTransform", signature = (time=None))]
            pub fn compute_local_to_world_transform(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mut cache = XformCache::new(t);
                let m = cache.get_local_to_world_transform(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).imageable().prim(),
                );
                Ok(crate::gf::matrix::PyMatrix4d(m))
            }
            #[pyo3(name = "ComputeParentToWorldTransform", signature = (time=None))]
            pub fn compute_parent_to_world_transform(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mut cache = XformCache::new(t);
                let m = cache.get_parent_to_world_transform(
                    crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).imageable().prim(),
                );
                Ok(crate::gf::matrix::PyMatrix4d(m))
            }
            #[staticmethod]
            #[pyo3(name = "GetOrderedPurposeTokens")]
            pub fn get_ordered_purpose_tokens() -> Vec<String> {
                Imageable::get_ordered_purpose_tokens()
                    .iter()
                    .map(|t| t.as_str().to_owned())
                    .collect()
            }
            #[pyo3(name = "IsValid")]
            pub fn is_valid_schema(&self) -> bool {
                crate::xform_img_delegate::GeomXformImg::geom_xf(&self.0).is_valid()
            }
        }
    };
}

/// `UsdGeomScope` — Imageable only (no xform stack).
macro_rules! usd_geom_schema_imageable_scope {
    ($py:ty, { $($body:tt)* }) => {
        #[pymethods]
        impl $py {
            $($body)*
            #[pyo3(name = "GetVisibilityAttr")]
            pub fn get_visibility_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(self.0.imageable().get_visibility_attr())
            }
            #[pyo3(name = "CreateVisibilityAttr")]
            pub fn create_visibility_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(self.0.imageable().create_visibility_attr())
            }
            #[pyo3(name = "GetPurposeAttr")]
            pub fn get_purpose_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(self.0.imageable().get_purpose_attr())
            }
            #[pyo3(name = "CreatePurposeAttr")]
            pub fn create_purpose_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(self.0.imageable().create_purpose_attr())
            }
            #[pyo3(name = "ComputeVisibility", signature = (time=None))]
            pub fn compute_visibility(&self, time: Option<f64>) -> String {
                self.0
                    .imageable()
                    .compute_visibility(tc(time))
                    .as_str()
                    .to_owned()
            }
            #[pyo3(name = "ComputePurpose")]
            pub fn compute_purpose(&self) -> String {
                self.0.imageable().compute_purpose().as_str().to_owned()
            }
            #[pyo3(name = "MakeVisible", signature = (time=None))]
            pub fn make_visible(&self, time: Option<f64>) {
                self.0.imageable().make_visible(tc(time));
            }
            #[pyo3(name = "MakeInvisible", signature = (time=None))]
            pub fn make_invisible(&self, time: Option<f64>) {
                self.0.imageable().make_invisible(tc(time));
            }
            #[pyo3(name = "ComputeWorldBound")]
            pub fn compute_world_bound(&self, time: f64, purpose: &str) -> Vec<f64> {
                let mut cache = BBoxCache::new(
                    TimeCode::new(time),
                    vec![Token::new(purpose)],
                    false,
                    false,
                );
                bbox_to_flat(&cache.compute_world_bound(self.0.imageable().prim()))
            }
            #[pyo3(name = "ComputeLocalBound")]
            pub fn compute_local_bound(&self, time: f64, purpose: &str) -> Vec<f64> {
                let mut cache = BBoxCache::new(
                    TimeCode::new(time),
                    vec![Token::new(purpose)],
                    false,
                    false,
                );
                bbox_to_flat(&cache.compute_local_bound(self.0.imageable().prim()))
            }
            #[pyo3(name = "ComputeLocalToWorldTransform", signature = (time=None))]
            pub fn compute_local_to_world_transform(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mut cache = XformCache::new(t);
                let m = cache.get_local_to_world_transform(self.0.imageable().prim());
                Ok(crate::gf::matrix::PyMatrix4d(m))
            }
            #[pyo3(name = "ComputeParentToWorldTransform", signature = (time=None))]
            pub fn compute_parent_to_world_transform(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mut cache = XformCache::new(t);
                let m = cache.get_parent_to_world_transform(self.0.imageable().prim());
                Ok(crate::gf::matrix::PyMatrix4d(m))
            }
            #[staticmethod]
            #[pyo3(name = "GetOrderedPurposeTokens")]
            pub fn get_ordered_purpose_tokens() -> Vec<String> {
                Imageable::get_ordered_purpose_tokens()
                    .iter()
                    .map(|t| t.as_str().to_owned())
                    .collect()
            }
            #[pyo3(name = "IsValid")]
            pub fn is_valid_schema(&self) -> bool {
                self.0.imageable().is_valid()
            }
        }
    };
}

/// `UsdGeomSubset` — Imageable via prim wrapper.
macro_rules! usd_geom_schema_imageable_subset {
    ($py:ty, { $($body:tt)* }) => {
        #[pymethods]
        impl $py {
            $($body)*
            #[pyo3(name = "GetVisibilityAttr")]
            pub fn get_visibility_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(imageable_for_subset_prim(self.0.prim()).get_visibility_attr())
            }
            #[pyo3(name = "CreateVisibilityAttr")]
            pub fn create_visibility_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(imageable_for_subset_prim(self.0.prim()).create_visibility_attr())
            }
            #[pyo3(name = "GetPurposeAttr")]
            pub fn get_purpose_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(imageable_for_subset_prim(self.0.prim()).get_purpose_attr())
            }
            #[pyo3(name = "CreatePurposeAttr")]
            pub fn create_purpose_attr(&self) -> PyAttribute {
                PyAttribute::from_attr(imageable_for_subset_prim(self.0.prim()).create_purpose_attr())
            }
            #[pyo3(name = "ComputeVisibility", signature = (time=None))]
            pub fn compute_visibility(&self, time: Option<f64>) -> String {
                imageable_for_subset_prim(self.0.prim())
                    .compute_visibility(tc(time))
                    .as_str()
                    .to_owned()
            }
            #[pyo3(name = "ComputePurpose")]
            pub fn compute_purpose(&self) -> String {
                imageable_for_subset_prim(self.0.prim())
                    .compute_purpose()
                    .as_str()
                    .to_owned()
            }
            #[pyo3(name = "MakeVisible", signature = (time=None))]
            pub fn make_visible(&self, time: Option<f64>) {
                imageable_for_subset_prim(self.0.prim()).make_visible(tc(time));
            }
            #[pyo3(name = "MakeInvisible", signature = (time=None))]
            pub fn make_invisible(&self, time: Option<f64>) {
                imageable_for_subset_prim(self.0.prim()).make_invisible(tc(time));
            }
            #[pyo3(name = "ComputeWorldBound")]
            pub fn compute_world_bound(&self, time: f64, purpose: &str) -> Vec<f64> {
                let mut cache = BBoxCache::new(
                    TimeCode::new(time),
                    vec![Token::new(purpose)],
                    false,
                    false,
                );
                let img = imageable_for_subset_prim(self.0.prim());
                bbox_to_flat(&cache.compute_world_bound(img.prim()))
            }
            #[pyo3(name = "ComputeLocalBound")]
            pub fn compute_local_bound(&self, time: f64, purpose: &str) -> Vec<f64> {
                let mut cache = BBoxCache::new(
                    TimeCode::new(time),
                    vec![Token::new(purpose)],
                    false,
                    false,
                );
                let img = imageable_for_subset_prim(self.0.prim());
                bbox_to_flat(&cache.compute_local_bound(img.prim()))
            }
            #[pyo3(name = "ComputeLocalToWorldTransform", signature = (time=None))]
            pub fn compute_local_to_world_transform(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mut cache = XformCache::new(t);
                let img = imageable_for_subset_prim(self.0.prim());
                let m = cache.get_local_to_world_transform(img.prim());
                Ok(crate::gf::matrix::PyMatrix4d(m))
            }
            #[pyo3(name = "ComputeParentToWorldTransform", signature = (time=None))]
            pub fn compute_parent_to_world_transform(
                &self,
                time: Option<&pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<crate::gf::matrix::PyMatrix4d> {
                let t = tc_from_py_opt(time)?;
                let mut cache = XformCache::new(t);
                let img = imageable_for_subset_prim(self.0.prim());
                let m = cache.get_parent_to_world_transform(img.prim());
                Ok(crate::gf::matrix::PyMatrix4d(m))
            }
            #[staticmethod]
            #[pyo3(name = "GetOrderedPurposeTokens")]
            pub fn get_ordered_purpose_tokens() -> Vec<String> {
                Imageable::get_ordered_purpose_tokens()
                    .iter()
                    .map(|t| t.as_str().to_owned())
                    .collect()
            }
            #[pyo3(name = "IsValid")]
            pub fn is_valid_schema(&self) -> bool {
                imageable_for_subset_prim(self.0.prim()).is_valid()
            }
        }
    };
}

