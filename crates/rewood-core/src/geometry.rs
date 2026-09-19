//! Minimal axis-aligned geometry for the MVP: every part is a rectangular
//! panel, placed in furniture space by a right-handed frame whose axes are
//! signed world axes. No rotations by arbitrary angles, no floating-point
//! trigonometry — positions stay exact and the output stays deterministic.
//!
//! Furniture space: X = width (left → right), Y = depth (back → front),
//! Z = height (bottom → top).
//!
//! Part space: X = length, Y = width, Z = thickness. Local `+Z` is the face
//! where machining is preferred — the inside of a carcass, the back of a door.

use std::ops::{Add, Mul, Sub};

use serde::{Deserialize, Serialize};

use crate::units::{approx_eq, EPS};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3(pub f64, pub f64, pub f64);

impl Vec3 {
    pub const ZERO: Vec3 = Vec3(0.0, 0.0, 0.0);

    pub fn dot(self, o: Vec3) -> f64 {
        self.0 * o.0 + self.1 * o.1 + self.2 * o.2
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3(
            self.1 * o.2 - self.2 * o.1,
            self.2 * o.0 - self.0 * o.2,
            self.0 * o.1 - self.1 * o.0,
        )
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn component(self, i: usize) -> f64 {
        match i {
            0 => self.0,
            1 => self.1,
            _ => self.2,
        }
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3(self.0 + o.0, self.1 + o.1, self.2 + o.2)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3(self.0 - o.0, self.1 - o.1, self.2 - o.2)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        Vec3(self.0 * s, self.1 * s, self.2 * s)
    }
}

/// A signed world axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl Axis {
    pub fn vec(self) -> Vec3 {
        match self {
            Axis::PosX => Vec3(1.0, 0.0, 0.0),
            Axis::NegX => Vec3(-1.0, 0.0, 0.0),
            Axis::PosY => Vec3(0.0, 1.0, 0.0),
            Axis::NegY => Vec3(0.0, -1.0, 0.0),
            Axis::PosZ => Vec3(0.0, 0.0, 1.0),
            Axis::NegZ => Vec3(0.0, 0.0, -1.0),
        }
    }

    pub fn from_vec(v: Vec3) -> Axis {
        match (v.0.round() as i8, v.1.round() as i8, v.2.round() as i8) {
            (1, 0, 0) => Axis::PosX,
            (-1, 0, 0) => Axis::NegX,
            (0, 1, 0) => Axis::PosY,
            (0, -1, 0) => Axis::NegY,
            (0, 0, 1) => Axis::PosZ,
            (0, 0, -1) => Axis::NegZ,
            _ => panic!("not a unit axis: {v:?}"),
        }
    }

    pub fn negate(self) -> Axis {
        match self {
            Axis::PosX => Axis::NegX,
            Axis::NegX => Axis::PosX,
            Axis::PosY => Axis::NegY,
            Axis::NegY => Axis::PosY,
            Axis::PosZ => Axis::NegZ,
            Axis::NegZ => Axis::PosZ,
        }
    }

    /// 0 for X, 1 for Y, 2 for Z.
    pub fn index(self) -> usize {
        match self {
            Axis::PosX | Axis::NegX => 0,
            Axis::PosY | Axis::NegY => 1,
            Axis::PosZ | Axis::NegZ => 2,
        }
    }

    pub fn sign(self) -> f64 {
        match self {
            Axis::PosX | Axis::PosY | Axis::PosZ => 1.0,
            _ => -1.0,
        }
    }
}

/// The six faces of a panel, named in the part's own frame.
/// `Front`/`Back` are the two large faces; the other four are the edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Face {
    /// Local +Z: the preferred machining face.
    Front,
    /// Local -Z.
    Back,
    /// Local -X.
    Left,
    /// Local +X.
    Right,
    /// Local -Y.
    Bottom,
    /// Local +Y.
    Top,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::Front,
        Face::Back,
        Face::Left,
        Face::Right,
        Face::Bottom,
        Face::Top,
    ];

    pub fn normal_local(self) -> Axis {
        match self {
            Face::Front => Axis::PosZ,
            Face::Back => Axis::NegZ,
            Face::Left => Axis::NegX,
            Face::Right => Axis::PosX,
            Face::Bottom => Axis::NegY,
            Face::Top => Axis::PosY,
        }
    }

    pub fn from_normal_local(a: Axis) -> Face {
        match a {
            Axis::PosZ => Face::Front,
            Axis::NegZ => Face::Back,
            Axis::NegX => Face::Left,
            Axis::PosX => Face::Right,
            Axis::NegY => Face::Bottom,
            Axis::PosY => Face::Top,
        }
    }

    pub fn is_large(self) -> bool {
        matches!(self, Face::Front | Face::Back)
    }

    pub fn opposite(self) -> Face {
        Face::from_normal_local(self.normal_local().negate())
    }

    /// The two in-plane axes of the face's (u, v) frame, in part space.
    /// u and v are always measured along the part's positive X/Y/Z, so a
    /// hole at (u, v) on `Back` sits exactly opposite (u, v) on `Front`.
    pub fn uv_axes(self) -> (Axis, Axis) {
        match self {
            Face::Front | Face::Back => (Axis::PosX, Axis::PosY),
            Face::Left | Face::Right => (Axis::PosY, Axis::PosZ),
            Face::Bottom | Face::Top => (Axis::PosX, Axis::PosZ),
        }
    }
}

/// Part dimensions in part space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Dims {
    pub length: f64,
    pub width: f64,
    pub thickness: f64,
}

impl Dims {
    pub fn along(self, a: Axis) -> f64 {
        match a.index() {
            0 => self.length,
            1 => self.width,
            _ => self.thickness,
        }
    }

    /// Point on `face` at (u, v), in part space.
    pub fn uv_to_local(self, face: Face, u: f64, v: f64) -> Vec3 {
        let (ua, va) = face.uv_axes();
        let n = face.normal_local();
        let mut p = [0.0; 3];
        p[ua.index()] = u;
        p[va.index()] = v;
        p[n.index()] = if n.sign() > 0.0 { self.along(n) } else { 0.0 };
        Vec3(p[0], p[1], p[2])
    }

    /// (u, v) of a part-space point projected onto `face`.
    pub fn local_to_uv(self, face: Face, p: Vec3) -> (f64, f64) {
        let (ua, va) = face.uv_axes();
        (p.component(ua.index()), p.component(va.index()))
    }

    /// Size of the (u, v) rectangle of `face`.
    pub fn face_extent(self, face: Face) -> (f64, f64) {
        let (ua, va) = face.uv_axes();
        (self.along(ua), self.along(va))
    }

    pub fn volume(self) -> f64 {
        self.length * self.width * self.thickness
    }
}

/// Right-handed frame placing a part in furniture space: `origin` is where
/// the part's local (0, 0, 0) corner lands; `x` and `y` are where local +X
/// and +Y point; local +Z is their cross product.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub origin: Vec3,
    pub x: Axis,
    pub y: Axis,
}

impl Placement {
    pub fn new(origin: Vec3, x: Axis, y: Axis) -> Placement {
        assert!(
            x.index() != y.index(),
            "placement axes must be orthogonal: {x:?} {y:?}"
        );
        Placement { origin, x, y }
    }

    pub fn z(&self) -> Axis {
        Axis::from_vec(self.x.vec().cross(self.y.vec()))
    }

    pub fn world_axis(&self, local: Axis) -> Axis {
        let a = match local.index() {
            0 => self.x,
            1 => self.y,
            _ => self.z(),
        };
        if local.sign() > 0.0 {
            a
        } else {
            a.negate()
        }
    }

    /// The local face whose outward normal points along `world` axis.
    pub fn face_facing(&self, world: Axis) -> Face {
        Face::ALL
            .into_iter()
            .find(|f| self.world_axis(f.normal_local()) == world)
            .expect("every world axis is some face normal")
    }

    pub fn to_world(&self, local: Vec3) -> Vec3 {
        self.origin + self.x.vec() * local.0 + self.y.vec() * local.1 + self.z().vec() * local.2
    }

    pub fn to_local(&self, world: Vec3) -> Vec3 {
        let d = world - self.origin;
        Vec3(
            d.dot(self.x.vec()),
            d.dot(self.y.vec()),
            d.dot(self.z().vec()),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn of_part(placement: &Placement, dims: Dims) -> Aabb {
        let a = placement.to_world(Vec3::ZERO);
        let b = placement.to_world(Vec3(dims.length, dims.width, dims.thickness));
        Aabb {
            min: Vec3(a.0.min(b.0), a.1.min(b.1), a.2.min(b.2)),
            max: Vec3(a.0.max(b.0), a.1.max(b.1), a.2.max(b.2)),
        }
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// Overlap box, or `None` when the boxes do not share positive volume.
    /// Touching faces are not an overlap.
    pub fn intersection(&self, o: &Aabb) -> Option<Aabb> {
        let min = Vec3(
            self.min.0.max(o.min.0),
            self.min.1.max(o.min.1),
            self.min.2.max(o.min.2),
        );
        let max = Vec3(
            self.max.0.min(o.max.0),
            self.max.1.min(o.max.1),
            self.max.2.min(o.max.2),
        );
        let s = max - min;
        if s.0 > EPS && s.1 > EPS && s.2 > EPS {
            Some(Aabb { min, max })
        } else {
            None
        }
    }

    /// Rectangle where a face of `self` (normal `axis`) touches a face of
    /// `o` with the opposite normal. Returns the 2D overlap as an Aabb that
    /// is flat along `axis`, or `None` if the faces are not coplanar or do
    /// not overlap with positive area.
    pub fn contact(&self, axis: Axis, o: &Aabb) -> Option<Aabb> {
        let i = axis.index();
        let (plane_self, plane_other) = if axis.sign() > 0.0 {
            (self.max.component(i), o.min.component(i))
        } else {
            (self.min.component(i), o.max.component(i))
        };
        if !approx_eq(plane_self, plane_other) {
            return None;
        }
        let mut min = [0.0; 3];
        let mut max = [0.0; 3];
        for k in 0..3 {
            if k == i {
                min[k] = plane_self;
                max[k] = plane_self;
            } else {
                min[k] = self.min.component(k).max(o.min.component(k));
                max[k] = self.max.component(k).min(o.max.component(k));
                if max[k] - min[k] <= EPS {
                    return None;
                }
            }
        }
        Some(Aabb {
            min: Vec3(min[0], min[1], min[2]),
            max: Vec3(max[0], max[1], max[2]),
        })
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_is_right_handed() {
        let p = Placement::new(Vec3::ZERO, Axis::PosX, Axis::PosY);
        assert_eq!(p.z(), Axis::PosZ);
        let p = Placement::new(Vec3::ZERO, Axis::PosZ, Axis::PosY);
        assert_eq!(p.z(), Axis::NegX);
        let p = Placement::new(Vec3::ZERO, Axis::NegZ, Axis::NegY);
        assert_eq!(p.z(), Axis::NegX);
    }

    #[test]
    fn world_local_roundtrip() {
        let p = Placement::new(Vec3(1000.0, 400.0, 800.0), Axis::NegZ, Axis::NegY);
        let local = Vec3(100.0, 50.0, 18.0);
        let w = p.to_world(local);
        assert_eq!(p.to_local(w), local);
        assert_eq!(w, Vec3(1000.0 - 18.0, 350.0, 700.0));
    }

    #[test]
    fn aabb_and_contact() {
        let dims = Dims {
            length: 800.0,
            width: 400.0,
            thickness: 18.0,
        };
        // Left side: length runs up, inside face (+Z local) faces +X.
        let side = Placement::new(Vec3(0.0, 400.0, 0.0), Axis::PosZ, Axis::NegY);
        let side_box = Aabb::of_part(&side, dims);
        assert_eq!(side_box.min, Vec3::ZERO);
        assert_eq!(side_box.max, Vec3(18.0, 400.0, 800.0));

        // Bottom panel sitting between the sides, on the floor.
        let bottom = Placement::new(Vec3(18.0, 0.0, 0.0), Axis::PosX, Axis::PosY);
        let bottom_box = Aabb::of_part(
            &bottom,
            Dims {
                length: 964.0,
                width: 400.0,
                thickness: 18.0,
            },
        );
        assert!(side_box.intersection(&bottom_box).is_none());
        let c = side_box.contact(Axis::PosX, &bottom_box).unwrap();
        assert_eq!(c.min, Vec3(18.0, 0.0, 0.0));
        assert_eq!(c.max, Vec3(18.0, 400.0, 18.0));
        assert!(side_box.contact(Axis::PosY, &bottom_box).is_none());
    }

    #[test]
    fn face_uv_mapping() {
        let dims = Dims {
            length: 800.0,
            width: 400.0,
            thickness: 18.0,
        };
        assert_eq!(
            dims.uv_to_local(Face::Front, 50.0, 100.0),
            Vec3(50.0, 100.0, 18.0)
        );
        assert_eq!(
            dims.uv_to_local(Face::Back, 50.0, 100.0),
            Vec3(50.0, 100.0, 0.0)
        );
        assert_eq!(
            dims.uv_to_local(Face::Right, 100.0, 9.0),
            Vec3(800.0, 100.0, 9.0)
        );
        assert_eq!(
            dims.uv_to_local(Face::Top, 300.0, 9.0),
            Vec3(300.0, 400.0, 9.0)
        );
        assert_eq!(
            dims.local_to_uv(Face::Left, Vec3(0.0, 100.0, 9.0)),
            (100.0, 9.0)
        );
        assert_eq!(dims.face_extent(Face::Left), (400.0, 18.0));
    }

    #[test]
    fn face_facing_world_axis() {
        let side = Placement::new(Vec3(0.0, 400.0, 0.0), Axis::PosZ, Axis::NegY);
        assert_eq!(side.face_facing(Axis::PosX), Face::Front);
        assert_eq!(side.face_facing(Axis::PosY), Face::Bottom);
        assert_eq!(side.face_facing(Axis::PosZ), Face::Right);
        assert_eq!(side.face_facing(Axis::NegZ), Face::Left);
    }
}
