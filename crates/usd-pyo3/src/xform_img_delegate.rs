//! Xformable + Imageable access for `UsdGeom` schema types (parity with pxr inheritance).
//!
//! Reference: C++ schema classes inherit `UsdGeomXformable` / `UsdGeomImageable`; Python exposes
//! combined methods on the concrete type (`Mesh.GetXformOpOrderAttr`, etc.).

use usd_geom::{
    BasisCurves, Boundable, Camera, Capsule, Capsule1, Cone, Cube, Curves, Cylinder, Cylinder1,
    Gprim, HermiteCurves, Mesh, NurbsCurves, NurbsPatch, Plane, PointBased, PointInstancer, Points,
    Sphere, TetMesh, Xformable,
};

/// Resolve `UsdGeomXformable` for a schema (composition chain in `usd-geom`).
pub trait GeomXformImg {
    fn geom_xf(&self) -> &Xformable;
}

impl GeomXformImg for Sphere {
    fn geom_xf(&self) -> &Xformable {
        self.gprim().boundable().xformable()
    }
}
impl GeomXformImg for Cube {
    fn geom_xf(&self) -> &Xformable {
        self.gprim().boundable().xformable()
    }
}
impl GeomXformImg for Cone {
    fn geom_xf(&self) -> &Xformable {
        self.gprim().boundable().xformable()
    }
}
impl GeomXformImg for Cylinder {
    fn geom_xf(&self) -> &Xformable {
        self.gprim().boundable().xformable()
    }
}
impl GeomXformImg for Cylinder1 {
    fn geom_xf(&self) -> &Xformable {
        self.as_cylinder().gprim().boundable().xformable()
    }
}
impl GeomXformImg for Capsule {
    fn geom_xf(&self) -> &Xformable {
        self.gprim().boundable().xformable()
    }
}
impl GeomXformImg for Capsule1 {
    fn geom_xf(&self) -> &Xformable {
        self.as_capsule().gprim().boundable().xformable()
    }
}
impl GeomXformImg for Plane {
    fn geom_xf(&self) -> &Xformable {
        self.gprim().boundable().xformable()
    }
}
impl GeomXformImg for Gprim {
    fn geom_xf(&self) -> &Xformable {
        self.boundable().xformable()
    }
}
impl GeomXformImg for Boundable {
    fn geom_xf(&self) -> &Xformable {
        self.xformable()
    }
}
impl GeomXformImg for Mesh {
    fn geom_xf(&self) -> &Xformable {
        self.point_based().gprim().boundable().xformable()
    }
}
impl GeomXformImg for PointBased {
    fn geom_xf(&self) -> &Xformable {
        self.gprim().boundable().xformable()
    }
}
impl GeomXformImg for Points {
    fn geom_xf(&self) -> &Xformable {
        self.point_based().gprim().boundable().xformable()
    }
}
impl GeomXformImg for Curves {
    fn geom_xf(&self) -> &Xformable {
        self.point_based().gprim().boundable().xformable()
    }
}
impl GeomXformImg for BasisCurves {
    fn geom_xf(&self) -> &Xformable {
        self.curves().point_based().gprim().boundable().xformable()
    }
}
impl GeomXformImg for NurbsCurves {
    fn geom_xf(&self) -> &Xformable {
        self.curves().point_based().gprim().boundable().xformable()
    }
}
impl GeomXformImg for HermiteCurves {
    fn geom_xf(&self) -> &Xformable {
        self.curves().point_based().gprim().boundable().xformable()
    }
}
impl GeomXformImg for NurbsPatch {
    fn geom_xf(&self) -> &Xformable {
        self.point_based().gprim().boundable().xformable()
    }
}
impl GeomXformImg for TetMesh {
    fn geom_xf(&self) -> &Xformable {
        self.point_based().gprim().boundable().xformable()
    }
}
impl GeomXformImg for Camera {
    fn geom_xf(&self) -> &Xformable {
        self.xformable()
    }
}
impl GeomXformImg for PointInstancer {
    fn geom_xf(&self) -> &Xformable {
        self.boundable().xformable()
    }
}
