//! USD Geometry tokens - commonly used string tokens for usdGeom module.
//!
//! Port of pxr/usd/usdGeom/tokens.h

use std::sync::OnceLock;
use usd_tf::Token;

/// USD Geometry module tokens.
pub struct UsdGeomTokens {
    /// "visibility" - visibility attribute name
    pub visibility: Token,
    /// "purpose" - purpose attribute name
    pub purpose: Token,
    /// "proxyPrim" - proxy prim relationship name
    pub proxy_prim: Token,
    /// "inherited" - inherited visibility value
    pub inherited: Token,
    /// "invisible" - invisible visibility value
    pub invisible: Token,
    /// "visible" - visible visibility value (for purpose visibility)
    pub visible: Token,
    /// "default" - default purpose value
    pub default_: Token,
    /// "render" - render purpose value
    pub render: Token,
    /// "proxy" - proxy purpose value
    pub proxy: Token,
    /// "guide" - guide purpose value
    pub guide: Token,
    /// "extent" - extent attribute name
    pub extent: Token,
    /// "doubleSided" - doubleSided attribute name
    pub double_sided: Token,
    /// "orientation" - orientation attribute name
    pub orientation: Token,
    /// "rightHanded" - rightHanded orientation value
    pub right_handed: Token,
    /// "leftHanded" - leftHanded orientation value
    pub left_handed: Token,
    /// "primvars:displayColor" - displayColor primvar name
    pub primvars_display_color: Token,
    /// "primvars:displayOpacity" - displayOpacity primvar name
    pub primvars_display_opacity: Token,
    /// "displayColor" - displayColor name (without primvars: prefix)
    pub display_color: Token,
    /// "displayOpacity" - displayOpacity name (without primvars: prefix)
    pub display_opacity: Token,
    /// "xformOpOrder" - xformOpOrder attribute name
    pub xform_op_order: Token,
    /// "interpolation" - interpolation metadata key
    pub interpolation: Token,
    /// "elementSize" - elementSize metadata key
    pub element_size: Token,
    /// "unauthoredValuesIndex" - metadata key for the unauthorized values index
    pub unauthored_values_index: Token,
    /// "constant" - constant interpolation value
    pub constant: Token,
    /// "uniform" - uniform interpolation value
    pub uniform: Token,
    /// "vertex" - vertex interpolation value
    pub vertex: Token,
    /// "varying" - varying interpolation value
    pub varying: Token,
    /// "faceVarying" - faceVarying interpolation value
    pub face_varying: Token,
    /// "guideVisibility" - guideVisibility attribute name
    pub guide_visibility: Token,
    /// "proxyVisibility" - proxyVisibility attribute name
    pub proxy_visibility: Token,
    /// "renderVisibility" - renderVisibility attribute name
    pub render_visibility: Token,
    /// "points" - points attribute name
    pub points: Token,
    /// "velocities" - velocities attribute name
    pub velocities: Token,
    /// "normals" - normals attribute name
    pub normals: Token,
    /// "faceVertexIndices" - faceVertexIndices attribute name
    pub face_vertex_indices: Token,
    /// "faceVertexCounts" - faceVertexCounts attribute name
    pub face_vertex_counts: Token,
    /// "subdivisionScheme" - subdivisionScheme attribute name
    pub subdivision_scheme: Token,
    /// "interpolateBoundary" - interpolateBoundary attribute name
    pub interpolate_boundary: Token,
    /// "faceVaryingLinearInterpolation" - faceVaryingLinearInterpolation attribute name
    pub face_varying_linear_interpolation: Token,
    /// "triangleSubdivisionRule" - triangleSubdivisionRule attribute name
    pub triangle_subdivision_rule: Token,
    /// "holeIndices" - holeIndices attribute name
    pub hole_indices: Token,
    /// "cornerIndices" - cornerIndices attribute name
    pub corner_indices: Token,
    /// "cornerSharpnesses" - cornerSharpnesses attribute name
    pub corner_sharpnesses: Token,
    /// "creaseIndices" - creaseIndices attribute name
    pub crease_indices: Token,
    /// "creaseLengths" - creaseLengths attribute name
    pub crease_lengths: Token,
    /// "creaseSharpnesses" - creaseSharpnesses attribute name
    pub crease_sharpnesses: Token,
    /// "catmullClark" - catmullClark subdivision scheme value
    pub catmull_clark: Token,
    /// "loop" - loop subdivision scheme value
    pub loop_: Token,
    /// "bilinear" - bilinear subdivision scheme value
    pub bilinear: Token,
    /// "none" - none subdivision scheme value
    pub none: Token,
    /// "edgeOnly" - edgeOnly interpolateBoundary value
    pub edge_only: Token,
    /// "edgeAndCorner" - edgeAndCorner interpolateBoundary value
    pub edge_and_corner: Token,
    /// "cornersOnly" - cornersOnly faceVaryingLinearInterpolation value
    pub corners_only: Token,
    /// "cornersPlus1" - cornersPlus1 faceVaryingLinearInterpolation value
    pub corners_plus1: Token,
    /// "cornersPlus2" - cornersPlus2 faceVaryingLinearInterpolation value
    pub corners_plus2: Token,
    /// "boundaries" - boundaries faceVaryingLinearInterpolation value
    pub boundaries: Token,
    /// "all" - all faceVaryingLinearInterpolation value
    pub all: Token,
    /// "smooth" - smooth triangleSubdivisionRule value
    pub smooth: Token,
    /// "widths" - widths attribute name
    pub widths: Token,
    /// "ids" - ids attribute name
    pub ids: Token,
    /// "curveVertexCounts" - curveVertexCounts attribute name
    pub curve_vertex_counts: Token,
    /// "radius" - radius attribute name
    pub radius: Token,
    /// "size" - size attribute name
    pub size: Token,
    /// "height" - height attribute name
    pub height: Token,
    /// "axis" - axis attribute name
    pub axis: Token,
    /// "width" - width attribute name (for Plane)
    pub width: Token,
    /// "length" - length attribute name (for Plane)
    pub length: Token,
    /// "radiusTop" - radiusTop attribute name
    pub radius_top: Token,
    /// "radiusBottom" - radiusBottom attribute name
    pub radius_bottom: Token,
    /// "X" - X axis value
    pub x: Token,
    /// "Y" - Y axis value
    pub y: Token,
    /// "Z" - Z axis value
    pub z: Token,
    /// "upAxis" - upAxis metadata key
    pub up_axis: Token,
    /// "metersPerUnit" - metersPerUnit metadata key
    pub meters_per_unit: Token,
    /// "type" - type attribute name
    pub type_: Token,
    /// "basis" - basis attribute name
    pub basis: Token,
    /// "wrap" - wrap attribute name
    pub wrap: Token,
    /// "linear" - linear type value
    pub linear: Token,
    /// "cubic" - cubic type value
    pub cubic: Token,
    /// "bezier" - bezier basis value
    pub bezier: Token,
    /// "bspline" - bspline basis value
    pub bspline: Token,
    /// "catmullRom" - catmullRom basis value
    pub catmull_rom: Token,
    /// "nonperiodic" - nonperiodic wrap value
    pub nonperiodic: Token,
    /// "periodic" - periodic wrap value
    pub periodic: Token,
    /// "pinned" - pinned wrap value
    pub pinned: Token,
    /// "tangents" - tangents attribute name
    pub tangents: Token,
    /// "order" - order attribute name
    pub order: Token,
    /// "knots" - knots attribute name
    pub knots: Token,
    /// "range" - range attribute name
    pub range: Token,
    /// "uVertexCount" - uVertexCount attribute name
    pub u_vertex_count: Token,
    /// "vVertexCount" - vVertexCount attribute name
    pub v_vertex_count: Token,
    /// "uOrder" - uOrder attribute name
    pub u_order: Token,
    /// "vOrder" - vOrder attribute name
    pub v_order: Token,
    /// "uKnots" - uKnots attribute name
    pub u_knots: Token,
    /// "vKnots" - vKnots attribute name
    pub v_knots: Token,
    /// "uRange" - uRange attribute name
    pub u_range: Token,
    /// "vRange" - vRange attribute name
    pub v_range: Token,
    /// "uForm" - uForm attribute name
    pub u_form: Token,
    /// "vForm" - vForm attribute name
    pub v_form: Token,
    /// "open" - open form value
    pub open: Token,
    /// "closed" - closed form value
    pub closed: Token,
    /// "ranges" - ranges attribute name
    pub ranges: Token,
    /// "pointWeights" - pointWeights attribute name
    pub point_weights: Token,
    /// "trimCurveCounts" - trimCurveCounts attribute name
    pub trim_curve_counts: Token,
    /// "trimCurveOrders" - trimCurveOrders attribute name
    pub trim_curve_orders: Token,
    /// "trimCurveVertexCounts" - trimCurveVertexCounts attribute name
    pub trim_curve_vertex_counts: Token,
    /// "trimCurveKnots" - trimCurveKnots attribute name
    pub trim_curve_knots: Token,
    /// "trimCurveRanges" - trimCurveRanges attribute name
    pub trim_curve_ranges: Token,
    /// "trimCurvePoints" - trimCurvePoints attribute name
    pub trim_curve_points: Token,
    /// "projection" - projection attribute name
    pub projection: Token,
    /// "horizontalAperture" - horizontalAperture attribute name
    pub horizontal_aperture: Token,
    /// "verticalAperture" - verticalAperture attribute name
    pub vertical_aperture: Token,
    /// "horizontalApertureOffset" - horizontalApertureOffset attribute name
    pub horizontal_aperture_offset: Token,
    /// "verticalApertureOffset" - verticalApertureOffset attribute name
    pub vertical_aperture_offset: Token,
    /// "focalLength" - focalLength attribute name
    pub focal_length: Token,
    /// "clippingRange" - clippingRange attribute name
    pub clipping_range: Token,
    /// "clippingPlanes" - clippingPlanes attribute name
    pub clipping_planes: Token,
    /// "fStop" - fStop attribute name
    pub f_stop: Token,
    /// "focusDistance" - focusDistance attribute name
    pub focus_distance: Token,
    /// "stereoRole" - stereoRole attribute name
    pub stereo_role: Token,
    /// "shutterOpen" - shutterOpen attribute name
    pub shutter_open: Token,
    /// "shutterClose" - shutterClose attribute name
    pub shutter_close: Token,
    /// "exposure" - exposure attribute name
    pub exposure: Token,
    /// "exposureIso" - exposureIso attribute name
    pub exposure_iso: Token,
    /// "exposureTime" - exposureTime attribute name
    pub exposure_time: Token,
    /// "exposureFStop" - exposureFStop attribute name
    pub exposure_f_stop: Token,
    /// "exposureResponsivity" - exposureResponsivity attribute name
    pub exposure_responsivity: Token,
    /// "perspective" - perspective projection value
    pub perspective: Token,
    /// "orthographic" - orthographic projection value
    pub orthographic: Token,
    /// "mono" - mono stereoRole value
    pub mono: Token,
    /// "left" - left stereoRole value
    pub left: Token,
    /// "right" - right stereoRole value
    pub right: Token,
    /// "tetVertexIndices" - tetVertexIndices attribute name
    pub tet_vertex_indices: Token,
    /// "surfaceFaceVertexIndices" - surfaceFaceVertexIndices attribute name
    pub surface_face_vertex_indices: Token,
    /// "indices" - indices attribute name
    pub indices: Token,
    /// "elementType" - elementType attribute name
    pub element_type: Token,
    /// "familyName" - familyName attribute name
    pub family_name: Token,
    /// "face" - face elementType value
    pub face: Token,
    /// "point" - point elementType value
    pub point: Token,
    /// "edge" - edge elementType value
    pub edge: Token,
    /// "segment" - segment elementType value
    pub segment: Token,
    /// "tetrahedron" - tetrahedron elementType value
    pub tetrahedron: Token,
    /// "partition" - partition familyType value
    pub partition: Token,
    /// "nonOverlapping" - nonOverlapping familyType value
    pub non_overlapping: Token,
    /// "unrestricted" - unrestricted familyType value
    pub unrestricted: Token,
    /// "positions" - positions attribute name
    pub positions: Token,
    /// "orientations" - orientations attribute name
    pub orientations: Token,
    /// "orientationsf" - orientationsf attribute name
    pub orientationsf: Token,
    /// "scales" - scales attribute name
    pub scales: Token,
    /// "protoIndices" - protoIndices attribute name
    pub proto_indices: Token,
    /// "angularVelocities" - angularVelocities attribute name
    pub angular_velocities: Token,
    /// "invisibleIds" - invisibleIds attribute name
    pub invisible_ids: Token,
    /// "inactiveIds" - inactiveIds metadata name
    pub inactive_ids: Token,
    /// "prototypes" - prototypes relationship name
    pub prototypes: Token,
    /// "accelerations" - accelerations attribute name
    pub accelerations: Token,
    /// "model:drawMode" - model drawMode attribute name
    pub model_draw_mode: Token,
    /// "model:applyDrawMode" - model applyDrawMode attribute name
    pub model_apply_draw_mode: Token,
    /// "model:drawModeColor" - model drawModeColor attribute name
    pub model_draw_mode_color: Token,
    /// "model:cardGeometry" - model cardGeometry attribute name
    pub model_card_geometry: Token,
    /// "model:cardTextureXPos" - model cardTextureXPos attribute name
    pub model_card_texture_x_pos: Token,
    /// "model:cardTextureYPos" - model cardTextureYPos attribute name
    pub model_card_texture_y_pos: Token,
    /// "model:cardTextureZPos" - model cardTextureZPos attribute name
    pub model_card_texture_z_pos: Token,
    /// "model:cardTextureXNeg" - model cardTextureXNeg attribute name
    pub model_card_texture_x_neg: Token,
    /// "model:cardTextureYNeg" - model cardTextureYNeg attribute name
    pub model_card_texture_y_neg: Token,
    /// "model:cardTextureZNeg" - model cardTextureZNeg attribute name
    pub model_card_texture_z_neg: Token,
    /// "extentsHint" - extentsHint attribute name
    pub extents_hint: Token,
    /// "motion:blurScale" - motion blurScale attribute name
    pub motion_blur_scale: Token,
    /// "motion:velocityScale" - motion velocityScale attribute name
    pub motion_velocity_scale: Token,
    /// "motion:nonlinearSampleCount" - motion nonlinearSampleCount attribute name
    pub motion_nonlinear_sample_count: Token,
    /// "origin" - origin drawMode value
    pub origin: Token,
    /// "bounds" - bounds drawMode value
    pub bounds: Token,
    /// "cards" - cards drawMode value
    pub cards: Token,
    /// "cross" - cross cardGeometry value
    pub cross: Token,
    /// "box" - box cardGeometry value
    pub r#box: Token,
    /// "fromTexture" - fromTexture cardGeometry value
    pub from_texture: Token,
    /// "constraintTargets" - constraintTargets namespace
    pub constraint_targets: Token,
    /// "constraintTargetIdentifier" - constraintTargetIdentifier metadata key
    pub constraint_target_identifier: Token,

    // ---- Schema type name tokens (C++ UsdGeomTokens::BasisCurves etc.) ----
    /// "BasisCurves" - schema type name for UsdGeomBasisCurves
    pub basis_curves_schema: Token,
    /// "Boundable" - schema type name for UsdGeomBoundable
    pub boundable_schema: Token,
    /// "Camera" - schema type name for UsdGeomCamera
    pub camera_schema: Token,
    /// "Capsule" - schema type name / family for UsdGeomCapsule
    pub capsule_schema: Token,
    /// "Capsule_1" - schema type name for UsdGeomCapsule_1
    pub capsule_1_schema: Token,
    /// "Cone" - schema type name for UsdGeomCone
    pub cone_schema: Token,
    /// "Cube" - schema type name for UsdGeomCube
    pub cube_schema: Token,
    /// "Curves" - schema type name for UsdGeomCurves
    pub curves_schema: Token,
    /// "Cylinder" - schema type name / family for UsdGeomCylinder
    pub cylinder_schema: Token,
    /// "Cylinder_1" - schema type name for UsdGeomCylinder_1
    pub cylinder_1_schema: Token,
    /// "GeomModelAPI" - schema type name for UsdGeomModelAPI
    pub geom_model_api_schema: Token,
    /// "GeomSubset" - schema type name for UsdGeomSubset
    pub geom_subset_schema: Token,
    /// "Gprim" - schema type name for UsdGeomGprim
    pub gprim_schema: Token,
    /// "HermiteCurves" - schema type name for UsdGeomHermiteCurves
    pub hermite_curves_schema: Token,
    /// "Imageable" - schema type name for UsdGeomImageable
    pub imageable_schema: Token,
    /// "Mesh" - schema type name for UsdGeomMesh
    pub mesh_schema: Token,
    /// "MotionAPI" - schema type name for UsdGeomMotionAPI
    pub motion_api_schema: Token,
    /// "NurbsCurves" - schema type name for UsdGeomNurbsCurves
    pub nurbs_curves_schema: Token,
    /// "NurbsPatch" - schema type name for UsdGeomNurbsPatch
    pub nurbs_patch_schema: Token,
    /// "Plane" - schema type name for UsdGeomPlane
    pub plane_schema: Token,
    /// "PointBased" - schema type name for UsdGeomPointBased
    pub point_based_schema: Token,
    /// "PointInstancer" - schema type name for UsdGeomPointInstancer
    pub point_instancer_schema: Token,
    /// "Points" - schema type name for UsdGeomPoints
    pub points_schema: Token,
    /// "PrimvarsAPI" - schema type name for UsdGeomPrimvarsAPI
    pub primvars_api_schema: Token,
    /// "Scope" - schema type name for UsdGeomScope
    pub scope_schema: Token,
    /// "Sphere" - schema type name for UsdGeomSphere
    pub sphere_schema: Token,
    /// "TetMesh" - schema type name for UsdGeomTetMesh
    pub tet_mesh_schema: Token,
    /// "VisibilityAPI" - schema type name for UsdGeomVisibilityAPI
    pub visibility_api_schema: Token,
    /// "Xform" - schema type name for UsdGeomXform
    pub xform_schema: Token,
    /// "Xformable" - schema type name for UsdGeomXformable
    pub xformable_schema: Token,
    /// "XformCommonAPI" - schema type name for UsdGeomXformCommonAPI
    pub xform_common_api_schema: Token,

    // ---- Deprecated basis tokens (kept for backward compat) ----
    /// "hermite" - deprecated basis value (HermiteCurves)
    pub hermite: Token,
    /// "power" - deprecated basis value
    pub power: Token,
}

impl UsdGeomTokens {
    fn new() -> Self {
        Self {
            visibility: Token::new("visibility"),
            purpose: Token::new("purpose"),
            proxy_prim: Token::new("proxyPrim"),
            inherited: Token::new("inherited"),
            invisible: Token::new("invisible"),
            visible: Token::new("visible"),
            default_: Token::new("default"),
            render: Token::new("render"),
            proxy: Token::new("proxy"),
            guide: Token::new("guide"),
            extent: Token::new("extent"),
            double_sided: Token::new("doubleSided"),
            orientation: Token::new("orientation"),
            right_handed: Token::new("rightHanded"),
            left_handed: Token::new("leftHanded"),
            primvars_display_color: Token::new("primvars:displayColor"),
            primvars_display_opacity: Token::new("primvars:displayOpacity"),
            display_color: Token::new("displayColor"),
            display_opacity: Token::new("displayOpacity"),
            xform_op_order: Token::new("xformOpOrder"),
            interpolation: Token::new("interpolation"),
            element_size: Token::new("elementSize"),
            unauthored_values_index: Token::new("unauthoredValuesIndex"),
            constant: Token::new("constant"),
            uniform: Token::new("uniform"),
            vertex: Token::new("vertex"),
            varying: Token::new("varying"),
            face_varying: Token::new("faceVarying"),
            guide_visibility: Token::new("guideVisibility"),
            proxy_visibility: Token::new("proxyVisibility"),
            render_visibility: Token::new("renderVisibility"),
            points: Token::new("points"),
            velocities: Token::new("velocities"),
            accelerations: Token::new("accelerations"),
            normals: Token::new("normals"),
            face_vertex_indices: Token::new("faceVertexIndices"),
            face_vertex_counts: Token::new("faceVertexCounts"),
            subdivision_scheme: Token::new("subdivisionScheme"),
            interpolate_boundary: Token::new("interpolateBoundary"),
            face_varying_linear_interpolation: Token::new("faceVaryingLinearInterpolation"),
            triangle_subdivision_rule: Token::new("triangleSubdivisionRule"),
            hole_indices: Token::new("holeIndices"),
            corner_indices: Token::new("cornerIndices"),
            corner_sharpnesses: Token::new("cornerSharpnesses"),
            crease_indices: Token::new("creaseIndices"),
            crease_lengths: Token::new("creaseLengths"),
            crease_sharpnesses: Token::new("creaseSharpnesses"),
            catmull_clark: Token::new("catmullClark"),
            loop_: Token::new("loop"),
            bilinear: Token::new("bilinear"),
            none: Token::new("none"),
            edge_only: Token::new("edgeOnly"),
            edge_and_corner: Token::new("edgeAndCorner"),
            corners_only: Token::new("cornersOnly"),
            corners_plus1: Token::new("cornersPlus1"),
            corners_plus2: Token::new("cornersPlus2"),
            boundaries: Token::new("boundaries"),
            all: Token::new("all"),
            smooth: Token::new("smooth"),
            widths: Token::new("widths"),
            ids: Token::new("ids"),
            curve_vertex_counts: Token::new("curveVertexCounts"),
            radius: Token::new("radius"),
            size: Token::new("size"),
            height: Token::new("height"),
            axis: Token::new("axis"),
            width: Token::new("width"),
            length: Token::new("length"),
            radius_top: Token::new("radiusTop"),
            radius_bottom: Token::new("radiusBottom"),
            x: Token::new("X"),
            y: Token::new("Y"),
            z: Token::new("Z"),
            type_: Token::new("type"),
            basis: Token::new("basis"),
            wrap: Token::new("wrap"),
            linear: Token::new("linear"),
            cubic: Token::new("cubic"),
            bezier: Token::new("bezier"),
            bspline: Token::new("bspline"),
            catmull_rom: Token::new("catmullRom"),
            nonperiodic: Token::new("nonperiodic"),
            periodic: Token::new("periodic"),
            pinned: Token::new("pinned"),
            tangents: Token::new("tangents"),
            order: Token::new("order"),
            knots: Token::new("knots"),
            range: Token::new("range"),
            u_vertex_count: Token::new("uVertexCount"),
            v_vertex_count: Token::new("vVertexCount"),
            u_order: Token::new("uOrder"),
            v_order: Token::new("vOrder"),
            u_knots: Token::new("uKnots"),
            v_knots: Token::new("vKnots"),
            u_range: Token::new("uRange"),
            v_range: Token::new("vRange"),
            u_form: Token::new("uForm"),
            v_form: Token::new("vForm"),
            open: Token::new("open"),
            closed: Token::new("closed"),
            ranges: Token::new("ranges"),
            point_weights: Token::new("pointWeights"),
            trim_curve_counts: Token::new("trimCurve:counts"),
            trim_curve_orders: Token::new("trimCurve:orders"),
            trim_curve_vertex_counts: Token::new("trimCurve:vertexCounts"),
            trim_curve_knots: Token::new("trimCurve:knots"),
            trim_curve_ranges: Token::new("trimCurve:ranges"),
            trim_curve_points: Token::new("trimCurve:points"),
            projection: Token::new("projection"),
            horizontal_aperture: Token::new("horizontalAperture"),
            vertical_aperture: Token::new("verticalAperture"),
            horizontal_aperture_offset: Token::new("horizontalApertureOffset"),
            vertical_aperture_offset: Token::new("verticalApertureOffset"),
            focal_length: Token::new("focalLength"),
            clipping_range: Token::new("clippingRange"),
            clipping_planes: Token::new("clippingPlanes"),
            f_stop: Token::new("fStop"),
            focus_distance: Token::new("focusDistance"),
            stereo_role: Token::new("stereoRole"),
            shutter_open: Token::new("shutter:open"),
            shutter_close: Token::new("shutter:close"),
            exposure: Token::new("exposure"),
            exposure_iso: Token::new("exposure:iso"),
            exposure_time: Token::new("exposure:time"),
            exposure_f_stop: Token::new("exposure:fStop"),
            exposure_responsivity: Token::new("exposure:responsivity"),
            perspective: Token::new("perspective"),
            orthographic: Token::new("orthographic"),
            mono: Token::new("mono"),
            left: Token::new("left"),
            right: Token::new("right"),
            tet_vertex_indices: Token::new("tetVertexIndices"),
            surface_face_vertex_indices: Token::new("surfaceFaceVertexIndices"),
            indices: Token::new("indices"),
            element_type: Token::new("elementType"),
            family_name: Token::new("familyName"),
            face: Token::new("face"),
            point: Token::new("point"),
            edge: Token::new("edge"),
            segment: Token::new("segment"),
            tetrahedron: Token::new("tetrahedron"),
            partition: Token::new("partition"),
            non_overlapping: Token::new("nonOverlapping"),
            unrestricted: Token::new("unrestricted"),
            positions: Token::new("positions"),
            orientations: Token::new("orientations"),
            orientationsf: Token::new("orientationsf"),
            scales: Token::new("scales"),
            proto_indices: Token::new("protoIndices"),
            angular_velocities: Token::new("angularVelocities"),
            invisible_ids: Token::new("invisibleIds"),
            inactive_ids: Token::new("inactiveIds"),
            prototypes: Token::new("prototypes"),
            model_draw_mode: Token::new("model:drawMode"),
            model_apply_draw_mode: Token::new("model:applyDrawMode"),
            model_draw_mode_color: Token::new("model:drawModeColor"),
            model_card_geometry: Token::new("model:cardGeometry"),
            model_card_texture_x_pos: Token::new("model:cardTextureXPos"),
            model_card_texture_y_pos: Token::new("model:cardTextureYPos"),
            model_card_texture_z_pos: Token::new("model:cardTextureZPos"),
            model_card_texture_x_neg: Token::new("model:cardTextureXNeg"),
            model_card_texture_y_neg: Token::new("model:cardTextureYNeg"),
            model_card_texture_z_neg: Token::new("model:cardTextureZNeg"),
            extents_hint: Token::new("extentsHint"),
            motion_blur_scale: Token::new("motion:blurScale"),
            motion_velocity_scale: Token::new("motion:velocityScale"),
            motion_nonlinear_sample_count: Token::new("motion:nonlinearSampleCount"),
            origin: Token::new("origin"),
            bounds: Token::new("bounds"),
            cards: Token::new("cards"),
            cross: Token::new("cross"),
            r#box: Token::new("box"),
            from_texture: Token::new("fromTexture"),
            constraint_targets: Token::new("constraintTargets"),
            constraint_target_identifier: Token::new("constraintTargetIdentifier"),
            up_axis: Token::new("upAxis"),
            meters_per_unit: Token::new("metersPerUnit"),

            // Schema type name tokens
            basis_curves_schema: Token::new("BasisCurves"),
            boundable_schema: Token::new("Boundable"),
            camera_schema: Token::new("Camera"),
            capsule_schema: Token::new("Capsule"),
            capsule_1_schema: Token::new("Capsule_1"),
            cone_schema: Token::new("Cone"),
            cube_schema: Token::new("Cube"),
            curves_schema: Token::new("Curves"),
            cylinder_schema: Token::new("Cylinder"),
            cylinder_1_schema: Token::new("Cylinder_1"),
            geom_model_api_schema: Token::new("GeomModelAPI"),
            geom_subset_schema: Token::new("GeomSubset"),
            gprim_schema: Token::new("Gprim"),
            hermite_curves_schema: Token::new("HermiteCurves"),
            imageable_schema: Token::new("Imageable"),
            mesh_schema: Token::new("Mesh"),
            motion_api_schema: Token::new("MotionAPI"),
            nurbs_curves_schema: Token::new("NurbsCurves"),
            nurbs_patch_schema: Token::new("NurbsPatch"),
            plane_schema: Token::new("Plane"),
            point_based_schema: Token::new("PointBased"),
            point_instancer_schema: Token::new("PointInstancer"),
            points_schema: Token::new("Points"),
            primvars_api_schema: Token::new("PrimvarsAPI"),
            scope_schema: Token::new("Scope"),
            sphere_schema: Token::new("Sphere"),
            tet_mesh_schema: Token::new("TetMesh"),
            visibility_api_schema: Token::new("VisibilityAPI"),
            xform_schema: Token::new("Xform"),
            xformable_schema: Token::new("Xformable"),
            xform_common_api_schema: Token::new("XformCommonAPI"),

            // Deprecated basis tokens
            hermite: Token::new("hermite"),
            power: Token::new("power"),
        }
    }
}

/// Global instance of UsdGeomTokens.
pub fn usd_geom_tokens() -> &'static UsdGeomTokens {
    static INSTANCE: OnceLock<UsdGeomTokens> = OnceLock::new();
    INSTANCE.get_or_init(UsdGeomTokens::new)
}
