//! Procedural mesh building. Everything in the scene is generated in code
//! so the repository needs no art assets and every run renders the exact
//! same geometry.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::{PI, TAU};

#[derive(Default, Clone)]
pub struct MeshData {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub col: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
}

impl MeshData {
    pub fn vertex(&mut self, p: Vec3, n: Vec3, uv: Vec2, c: [f32; 4]) -> u32 {
        self.pos.push(p.to_array());
        self.nrm.push(n.to_array());
        self.uv.push(uv.to_array());
        self.col.push(c);
        (self.pos.len() - 1) as u32
    }

    pub fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.idx.extend_from_slice(&[a, b, c]);
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    /// Appends `other` transformed by `m`.
    pub fn append(&mut self, other: &MeshData, m: Mat4) {
        let base = self.pos.len() as u32;
        let nm = Mat3::from_mat4(m).inverse().transpose();
        for i in 0..other.pos.len() {
            self.pos
                .push(m.transform_point3(Vec3::from(other.pos[i])).to_array());
            self.nrm
                .push((nm * Vec3::from(other.nrm[i])).normalize_or_zero().to_array());
        }
        self.uv.extend_from_slice(&other.uv);
        self.col.extend_from_slice(&other.col);
        self.idx.extend(other.idx.iter().map(|i| i + base));
    }

    /// Sets every vertex color.
    pub fn paint(mut self, c: [f32; 4]) -> Self {
        self.col.iter_mut().for_each(|v| *v = c);
        self
    }

    /// Area weighted smooth normals from the index buffer.
    pub fn recompute_normals(&mut self) {
        let mut acc = vec![Vec3::ZERO; self.pos.len()];
        for t in self.idx.as_chunks::<3>().0 {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let pa = Vec3::from(self.pos[a]);
            let n = (Vec3::from(self.pos[b]) - pa).cross(Vec3::from(self.pos[c]) - pa);
            acc[a] += n;
            acc[b] += n;
            acc[c] += n;
        }
        self.nrm = acc
            .into_iter()
            .map(|n| n.normalize_or(Vec3::Y).to_array())
            .collect();
    }

    pub fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, Default::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Icosphere with `subdiv` subdivisions, radius 1.
pub fn icosphere(subdiv: u32) -> MeshData {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut verts: Vec<Vec3> = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .iter()
    .map(|v| Vec3::from(*v).normalize())
    .collect();
    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..subdiv {
        let mut cache = std::collections::HashMap::new();
        let mut mid = |a: u32, b: u32, verts: &mut Vec<Vec3>| -> u32 {
            let key = (a.min(b), a.max(b));
            *cache.entry(key).or_insert_with(|| {
                verts.push(((verts[a as usize] + verts[b as usize]) * 0.5).normalize());
                (verts.len() - 1) as u32
            })
        };
        let mut next = Vec::with_capacity(faces.len() * 4);
        for [a, b, c] in faces {
            let ab = mid(a, b, &mut verts);
            let bc = mid(b, c, &mut verts);
            let ca = mid(c, a, &mut verts);
            next.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        faces = next;
    }
    let mut m = MeshData::default();
    for v in &verts {
        let uv = Vec2::new(0.5 + v.z.atan2(v.x) / TAU, 0.5 - v.y.asin() / PI);
        m.vertex(*v, *v, uv, WHITE);
    }
    for [a, b, c] in faces {
        m.tri(a, b, c);
    }
    m
}

/// Open-ended tapered cylinder along +Y from y=0 to y=h.
pub fn cylinder(r0: f32, r1: f32, h: f32, sides: u32, rings: u32) -> MeshData {
    let mut m = MeshData::default();
    let slope = (r0 - r1) / h;
    for j in 0..=rings {
        let v = j as f32 / rings as f32;
        let r = r0 + (r1 - r0) * v;
        for i in 0..=sides {
            let a = i as f32 / sides as f32 * TAU;
            let (s, c) = a.sin_cos();
            let n = Vec3::new(c, slope, s).normalize();
            m.vertex(
                Vec3::new(c * r, v * h, s * r),
                n,
                Vec2::new(i as f32 / sides as f32, v * h),
                WHITE,
            );
        }
    }
    let row = sides + 1;
    for j in 0..rings {
        for i in 0..sides {
            let a = j * row + i;
            let b = a + row;
            m.tri(a, b, a + 1);
            m.tri(a + 1, b, b + 1);
        }
    }
    m
}

/// Cone along +Y with base at y=0, apex at y=h.
pub fn cone(r: f32, h: f32, sides: u32) -> MeshData {
    let mut m = MeshData::default();
    let apex = m.vertex(Vec3::Y * h, Vec3::Y, Vec2::new(0.5, 0.0), WHITE);
    for i in 0..=sides {
        let a = i as f32 / sides as f32 * TAU;
        let (s, c) = a.sin_cos();
        let n = Vec3::new(c * h, r, s * h).normalize();
        m.vertex(
            Vec3::new(c * r, 0.0, s * r),
            n,
            Vec2::new(i as f32 / sides as f32, 1.0),
            WHITE,
        );
    }
    for i in 0..sides {
        m.tri(apex, i + 2, i + 1);
    }
    // Underside so cones read as solid foliage from below.
    let center = m.vertex(Vec3::ZERO, -Vec3::Y, Vec2::splat(0.5), WHITE);
    for i in 0..sides {
        m.tri(center, i + 1, i + 2);
    }
    m
}

/// Unit quad in the XY plane centered at origin, facing +Z.
pub fn quad() -> MeshData {
    let mut m = MeshData::default();
    let n = Vec3::Z;
    let a = m.vertex(Vec3::new(-0.5, -0.5, 0.0), n, Vec2::new(0.0, 1.0), WHITE);
    let b = m.vertex(Vec3::new(0.5, -0.5, 0.0), n, Vec2::new(1.0, 1.0), WHITE);
    let c = m.vertex(Vec3::new(0.5, 0.5, 0.0), n, Vec2::new(1.0, 0.0), WHITE);
    let d = m.vertex(Vec3::new(-0.5, 0.5, 0.0), n, Vec2::new(0.0, 0.0), WHITE);
    m.tri(a, b, c);
    m.tri(a, c, d);
    m
}

/// Transform that maps the +Y unit axis onto the segment a -> b.
pub fn segment(a: Vec3, b: Vec3) -> Mat4 {
    let d = b - a;
    let rot = Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y));
    Mat4::from_rotation_translation(rot, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icosphere_counts() {
        let m = icosphere(2);
        assert_eq!(m.idx.len() / 3, 320);
        assert!(m.pos.iter().all(|p| (Vec3::from(*p).length() - 1.0).abs() < 1e-4));
    }
}
