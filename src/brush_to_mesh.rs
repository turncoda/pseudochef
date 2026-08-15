//! Convert Quake-style brushes into UE-space triangulated meshes.
//! Largely written using Claude Code with access to Unreal Engine 5.1 source code.

use crate::texture_collection::TextureCollection;
use glam::{DMat3, DVec2, DVec3, Vec3};
use itertools::Itertools;
use spade::{DelaunayTriangulation, Point2, Triangulation};
use std::collections::HashMap;

fn dvec3(p: shalrath::repr::Point) -> DVec3 {
    DVec3::new(p.x as f64, p.y as f64, p.z as f64)
}
fn calculate_normal(plane: &shalrath::repr::TrianglePlane) -> DVec3 {
    let p0 = dvec3(plane.v0);
    let p1 = dvec3(plane.v1);
    let p2 = dvec3(plane.v2);
    let a = p2 - p0;
    let b = p1 - p0;
    a.cross(b)
}

/// Assumption: input brush is convex in a left-handed (UE) coordinate system.
/// Otherwise, behavior is undefined.
fn compute_vertices(brush: &shalrath::repr::Brush) -> Vec<Vec<DVec3>> {
    let planes: Vec<_> = brush.0.iter().map(|plane| plane.plane).collect();
    let normals: Vec<_> = planes.iter().map(calculate_normal).collect();

    // Determinant threshold for treating three plane normals as linearly independent
    // (i.e. the planes meet at a unique point rather than being parallel/degenerate).
    const DET_EPSILON: f64 = 1e-6;

    // World-space distance threshold used both to decide whether a candidate point
    // actually lies inside every plane's half-space, and to dedupe vertices that
    // more than 3 planes meet at (e.g. a chamfered corner or a pyramid apex).
    const VERTEX_EPSILON: f64 = 1e-2;

    let mut vertices = vec![vec![]; brush.0.len()];
    for c in planes
        .iter()
        .zip(normals.iter())
        .enumerate()
        .combinations(3)
    {
        let (i0, (p0, n0)) = c[0];
        let (i1, (p1, n1)) = c[1];
        let (i2, (p2, n2)) = c[2];
        let x0 = dvec3(p0.v0);
        let x1 = dvec3(p1.v0);
        let x2 = dvec3(p2.v0);
        let u0 = n0.normalize();
        let u1 = n1.normalize();
        let u2 = n2.normalize();
        let det = DMat3::from_cols(u0, u1, u2).determinant();
        if det.abs() < DET_EPSILON {
            // Normals are (near-)parallel: these three planes don't meet at a
            // single point, so there's nothing to intersect.
            continue;
        }
        let c0 = x0.dot(u0) * u1.cross(u2);
        let c1 = x1.dot(u1) * u2.cross(u0);
        let c2 = x2.dot(u2) * u0.cross(u1);
        let intersection = det.powi(-1) * (c0 + c1 + c2);

        // A candidate point is only a real vertex of the (convex) brush if it
        // lies on the interior side of every plane, not just the three we
        // solved with. This is what makes the intersection test work for any
        // convex brush, not just ones where faces meet at right angles.
        let is_vertex = planes.iter().zip(normals.iter()).all(|(plane, normal)| {
            let x = dvec3(plane.v0);
            let n = normal.normalize();
            // n points inward, so interior points have a non-negative projection onto it.
            (intersection - x).dot(n) >= -VERTEX_EPSILON
        });
        if !is_vertex {
            continue;
        }

        for &i in &[i0, i1, i2] {
            let face_vertices = &mut vertices[i];
            // If more than 3 planes intersect at the same vertex, there will be duplicates.
            // This is unlikely with brushes but we account for it anyway.
            let is_duplicate = face_vertices
                .iter()
                .any(|v: &DVec3| v.distance(intersection) < VERTEX_EPSILON);
            if !is_duplicate {
                face_vertices.push(intersection);
            }
        }
    }
    vertices
}

pub struct AxisAlignedBoundingBox {
    pub origin: DVec3,
    pub extents: DVec3,
}

pub fn get_aabb(brush: &shalrath::repr::Brush) -> AxisAlignedBoundingBox {
    let vertices = compute_vertices(brush);
    let mut min = DVec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = DVec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    vertices.into_iter().flatten().for_each(|v| {
        min = min.min(v);
        max = max.max(v);
    });
    let origin = (max + min) / 2.0;
    let extents = (max - min) / 2.0;
    AxisAlignedBoundingBox { origin, extents }
}

fn derive_uvs(
    plane: &shalrath::repr::BrushPlane,
    points: &[DVec3],
    texture_collection: &TextureCollection,
) -> Vec<DVec2> {
    let texture_name: &str = plane.texture.as_ref();
    let texture = texture_collection.get(texture_name);
    if let shalrath::repr::TextureOffset::Valve { u, v } = plane.texture_offset {
        let du: DVec3 = Vec3::new(u.x, u.y, u.z).into();
        let dv: DVec3 = Vec3::new(v.x, v.y, v.z).into();
        let u_offset = u.d as f64;
        let v_offset = v.d as f64;
        let x_scale = plane.scale_x as f64;
        let y_scale = plane.scale_y as f64;
        points
            .iter()
            .map(|p| {
                DVec2::new(
                    (p.dot(du) / x_scale + u_offset) / texture.width as f64,
                    (p.dot(dv) / y_scale + v_offset) / texture.height as f64,
                )
            })
            .collect()
    } else {
        vec![DVec2::default(); points.len()]
    }
}

/// `target_vertex_spacing` controls triangulation density. Higher = more sparse.
/// Returns the triangulated mesh and the origin it is relative to.
pub fn convert_to_mesh(
    brush: &shalrath::repr::Brush,
    target_vertex_spacing: f64,
    texture_collection: &TextureCollection,
) -> (pseudocooker::MeshInput, DVec3) {
    let vertices = compute_vertices(&brush);

    // Deterministically pick a vertex to be the origin.
    let origin = vertices
        .iter()
        .flatten()
        .copied()
        .min_by(|a, b| (a.x, a.y, a.z).partial_cmp(&(b.x, b.y, b.z)).unwrap())
        .expect("a convex brush has at least one vertex");

    let planes: Vec<_> = brush.0.iter().collect();

    // Inward normals.
    let normals: Vec<_> = brush
        .0
        .iter()
        .map(|plane| &plane.plane)
        .map(calculate_normal)
        .collect();
    let true_outward_normals: Vec<DVec3> = normals.iter().map(|n| -n).collect();

    let mut positions = vec![];
    let mut faces = vec![];
    let mut mesh_normals = vec![];
    let mut material_names = vec!["/pseudochef/failed/to/find/material".to_string()];
    let mut mesh_uvs = vec![];
    // TODO All of this can be derived from the plane index;
    // eventually we should do it in the loop instead of before it.
    for (i, ((normal, face_vertices), plane)) in normals
        .iter()
        .zip(vertices.iter())
        .zip(planes.iter())
        .enumerate()
    {
        let base = positions.len();
        let (points, triangles) = triangulate_face(*normal, face_vertices, target_vertex_spacing);
        let uvs = derive_uvs(&plane, &points, texture_collection);
        mesh_uvs.extend(uvs);
        positions.extend(points);
        let n = true_outward_normals[i].normalize();
        let normal_index = mesh_normals.len();
        mesh_normals.push([n.x, n.y, n.z]);
        // TODO re-use repeat materials (probably not a big deal either way)
        let material_index =
            if let Some(material_path) = texture_collection.get(&plane.texture).material_path {
                material_names.push(material_path);
                material_names.len() - 1
            } else {
                0
            };
        for [a, b, c] in triangles {
            faces.push(pseudocooker::Face {
                material_index,
                corners: vec![
                    pseudocooker::Corner::new(base + a)
                        .with_normal(normal_index)
                        .with_uv(base + a),
                    pseudocooker::Corner::new(base + b)
                        .with_normal(normal_index)
                        .with_uv(base + b),
                    pseudocooker::Corner::new(base + c)
                        .with_normal(normal_index)
                        .with_uv(base + c),
                ],
            });
        }
    }

    let positions = positions.iter().map(|p| [p.x, p.y, p.z]).collect();
    let mesh_uvs = mesh_uvs.iter().map(|c| [c.x, c.y]).collect();

    let mut mesh = pseudocooker::MeshInput {
        positions,
        uvs: mesh_uvs,
        normals: mesh_normals,
        faces,
        material_names,
    };

    assert_eq!(
        mesh.positions.len(),
        mesh.uvs.len(),
        "each point should have a unique corresponding uv"
    );

    // Adjacent faces of the brush agree on where a shared edge's vertices
    // should be (same edge length, same spacing, so the same subdivision
    // points), but each face reconstructs its own vertex positions from its
    // own 2D projection, so shared vertices come out merely float-close
    // rather than bit-identical. Weld them so the mesh is actually
    // watertight instead of having micro-cracks along every brush edge.
    weld_vertices(&mut mesh, WELD_EPSILON);
    let boundary_edges = find_boundary_edges(&mesh);
    debug_assert!(
        boundary_edges.is_empty(),
        "generated mesh has holes/non-manifold edges: {boundary_edges:?}"
    );

    for p in &mut mesh.positions {
        p[0] -= origin.x;
        p[1] -= origin.y;
        p[2] -= origin.z;
    }

    (mesh, origin)
}

/// Returns every edge that isn't shared by exactly two triangles: for a
/// closed, watertight mesh built from a convex brush, each edge should be
/// used once by each of the two faces that meet there. An edge used only
/// once is a hole (or a crack left by an unwelded vertex); an edge used more
/// than twice indicates overlapping/degenerate triangles.
fn find_boundary_edges(mesh: &pseudocooker::MeshInput) -> Vec<(usize, usize)> {
    let mut edge_counts: HashMap<(usize, usize), u32> = HashMap::new();
    for face in &mesh.faces {
        let n = face.corners.len();
        for i in 0..n {
            let a = face.corners[i].position;
            let b = face.corners[(i + 1) % n].position;
            let edge = if a < b { (a, b) } else { (b, a) };
            *edge_counts.entry(edge).or_insert(0) += 1;
        }
    }
    edge_counts
        .into_iter()
        .filter(|&(_, count)| count != 2)
        .map(|(edge, _)| edge)
        .collect()
}

// World-space distance below which two vertices are considered the same
// point; matches the tolerance already used to dedupe brush-corner
// candidates above.
const WELD_EPSILON: f64 = 1e-2;

/// Merges mesh vertices that are within `epsilon` of each other in place,
/// remapping face corners to point at the surviving vertex.
fn weld_vertices(mesh: &mut pseudocooker::MeshInput, epsilon: f64) {
    let cell_size = epsilon;
    let cell_of = |p: [f64; 3]| -> (i64, i64, i64) {
        (
            (p[0] / cell_size).floor() as i64,
            (p[1] / cell_size).floor() as i64,
            (p[2] / cell_size).floor() as i64,
        )
    };

    let mut welded_positions: Vec<[f64; 3]> = vec![];
    let mut buckets: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    let mut remap: Vec<usize> = Vec::with_capacity(mesh.positions.len());

    for &p in &mesh.positions {
        let (cx, cy, cz) = cell_of(p);
        let mut existing = None;
        'search: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(candidates) = buckets.get(&(cx + dx, cy + dy, cz + dz)) else {
                        continue;
                    };
                    for &j in candidates {
                        let q = welded_positions[j];
                        let d2 =
                            (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2);
                        if d2 < epsilon * epsilon {
                            existing = Some(j);
                            break 'search;
                        }
                    }
                }
            }
        }
        let index = existing.unwrap_or_else(|| {
            let index = welded_positions.len();
            welded_positions.push(p);
            buckets.entry((cx, cy, cz)).or_default().push(index);
            index
        });
        remap.push(index);
    }

    mesh.positions = welded_positions;
    for face in &mut mesh.faces {
        for corner in &mut face.corners {
            corner.position = remap[corner.position];
        }
    }
}

/// Triangulates one convex, planar face (given its unordered boundary
/// vertices and outward normal) so that the resulting vertices are roughly
/// uniformly distributed, rather than fanned out from a single corner.
///
/// The face is convex by construction (see the plane-intersection loop
/// above), which means a plain Delaunay triangulation of its (subdivided)
/// boundary plus some interior sample points already spans exactly the
/// face's polygon -- unlike an arbitrary polygon, no constrained edges are
/// needed to keep the triangulation from crossing the boundary. The
/// boundary edges themselves are subdivided at the same spacing as the
/// interior, so triangles along the edge of a face are comparable in size
/// to interior ones instead of being long slivers -- important since that's
/// exactly where adjacent brushes meet.
///
/// One wrinkle: boundary subdivision deliberately places points exactly on
/// the straight line between the corners they subdivide, and `spade`'s
/// Delaunay triangulation isn't numerically stable for such near-collinear
/// input -- it can emit a spurious near-zero-area sliver triangle across a
/// corner and one of its own subdivision points, *in addition to* an
/// already-complete, correct triangulation of the rest of the polygon. That
/// extra triangle's edges don't correspond to real brush geometry and never
/// get matched by a neighboring triangle, leaving a hole. See the
/// `is_degenerate` filter below, which drops such triangles.
///
/// Returns the (deduplicated) points used -- boundary corners and edge
/// subdivisions first, in polygon order, followed by any generated interior
/// points -- and a list of triangles as indices into that point list.
fn triangulate_face(
    normal: DVec3,
    boundary: &[DVec3],
    target_spacing: f64,
) -> (Vec<DVec3>, Vec<[usize; 3]>) {
    if boundary.len() < 3 {
        return (boundary.to_vec(), vec![]);
    }

    let n = normal.normalize();
    // Build an orthonormal (u, v) basis for the face's plane such that
    // u.cross(v) == n: a counterclockwise triangle in (u, v) coordinates
    // then has outward normal n, matching the winding the rest of the
    // pipeline expects.
    let reference = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let u = n.cross(reference).normalize();
    let v = n.cross(u);
    let origin = boundary[0];
    let project = |p: DVec3| -> (f64, f64) {
        let d = p - origin;
        (d.dot(u), d.dot(v))
    };
    let unproject = |(x, y): (f64, f64)| -> DVec3 { origin + u * x + v * y };

    // Keep each corner's original (pre-projection) 3D position alongside its
    // 2D projection: two faces sharing an edge hold bit-identical DVec3
    // endpoints for it (see the plane-intersection loop above), so measuring
    // edge length in 3D -- rather than off each face's own reprojected 2D
    // coordinates, which can differ by a few ULPs between faces -- gives both
    // faces the exact same length and therefore the same subdivision segment
    // count. Without this, a length that lands a hair on either side of a
    // `spacing * (k + 0.5)` rounding boundary can subdivide to a different
    // number of points on each side of the edge, leaving a hole that welding
    // can't fix since the two sides don't even agree on point count.
    let mut corners: Vec<((f64, f64), DVec3)> = boundary.iter().map(|&p| (project(p), p)).collect();
    let centroid = {
        let sum = corners
            .iter()
            .fold((0.0, 0.0), |acc, (p, _)| (acc.0 + p.0, acc.1 + p.1));
        (sum.0 / corners.len() as f64, sum.1 / corners.len() as f64)
    };
    corners.sort_by(|(a, _), (b, _)| {
        let ta = (a.1 - centroid.1).atan2(a.0 - centroid.0);
        let tb = (b.1 - centroid.1).atan2(b.0 - centroid.0);
        ta.partial_cmp(&tb).unwrap()
    });
    let corners_2d: Vec<(f64, f64)> = corners.iter().map(|&(p, _)| p).collect();
    let corners_3d: Vec<DVec3> = corners.iter().map(|&(_, p)| p).collect();
    let boundary_2d = subdivide_boundary(&corners_2d, &corners_3d, target_spacing);

    let mut points_2d = boundary_2d.clone();
    if target_spacing > 0.0 {
        points_2d.extend(sample_interior_points(&boundary_2d, target_spacing));
    }

    let mut triangulation: DelaunayTriangulation<Point2<f64>> = DelaunayTriangulation::new();
    let mut index_of: HashMap<(u64, u64), usize> = HashMap::new();
    for (i, &(x, y)) in points_2d.iter().enumerate() {
        let (x64, y64) = (x, y);
        triangulation
            .insert(Point2::new(x64, y64))
            .expect("face sample points are finite and deduplicated by construction");
        index_of.insert((x64.to_bits(), y64.to_bits()), i);
    }

    let points: Vec<DVec3> = points_2d.iter().map(|&p| unproject(p)).collect();
    // A triangle is considered degenerate (and dropped below) if its
    // longest-edge "height" -- the perpendicular distance from the third
    // vertex to the line through the other two -- is under this tolerance.
    // Boundary subdivision places points exactly on the straight line
    // between the corners they subdivide, and spade's Delaunay
    // triangulation isn't numerically stable for such near-collinear input:
    // it can emit a spurious near-zero-area triangle straddling a corner
    // and one of its own subdivision points, *in addition to* (not instead
    // of) a full, correct, gapless triangulation of the rest of the
    // polygon. That extra triangle's edges don't correspond to any real
    // brush geometry and never get matched by a neighboring triangle,
    // leaving the watertightness check with a hole. Filtering it out here
    // (rather than trying to coax the triangulator into never producing it)
    // is safe because, by construction, it contributes ~0 area -- the
    // remaining triangles already fully tile the polygon without it.
    const DEGENERATE_TRIANGLE_HEIGHT_EPSILON: f64 = 1e-2;
    let is_degenerate = |a: DVec3, b: DVec3, c: DVec3| -> bool {
        let area2 = (b - a).cross(c - a).length();
        let longest_edge = (b - a).length().max((c - b).length()).max((a - c).length());
        if longest_edge < 1e-9 {
            return true;
        }
        area2 / longest_edge < DEGENERATE_TRIANGLE_HEIGHT_EPSILON
    };
    let triangles: Vec<[usize; 3]> = triangulation
        .inner_faces()
        .map(|face| {
            let [a, b, c] = face
                .positions()
                .map(|p| index_of[&(p.x.to_bits(), p.y.to_bits())]);
            // Delaunay triangulation is agnostic to our winding convention;
            // flip back to outward-facing if this triangle came out reversed.
            if (points[b] - points[a]).cross(points[c] - points[a]).dot(n) < 0.0 {
                [a, c, b]
            } else {
                [a, b, c]
            }
        })
        .filter(|&[a, b, c]| !is_degenerate(points[a], points[b], points[c]))
        .collect();

    (points, triangles)
}

/// Subdivides each edge of a (counterclockwise-ordered) polygon boundary
/// into segments of length as close to `spacing` as possible, so the
/// boundary carries roughly the same vertex density as the interior lattice
/// generated by `sample_interior_points`. A `spacing` <= 0.0 disables this
/// and returns the corners unchanged.
///
/// `corners_3d` gives each corner's original (pre-projection) position, in
/// the same order as `corners_ccw`; segment counts are derived from these
/// rather than from the projected 2D coordinates so that two faces sharing
/// an edge -- which reproject the same 3D endpoints through different bases
/// -- always agree on how many segments that edge gets. See the comment in
/// `triangulate_face` for why that agreement matters.
fn subdivide_boundary(
    corners_ccw: &[(f64, f64)],
    corners_3d: &[DVec3],
    spacing: f64,
) -> Vec<(f64, f64)> {
    if spacing <= 0.0 {
        return corners_ccw.to_vec();
    }
    let n = corners_ccw.len();
    let mut points = Vec::with_capacity(n);
    for i in 0..n {
        let a = corners_ccw[i];
        let b = corners_ccw[(i + 1) % n];
        points.push(a);
        let edge_len = corners_3d[i].distance(corners_3d[(i + 1) % n]);
        let segments = (edge_len / spacing).round().max(1.0) as usize;
        for k in 1..segments {
            let t = k as f64 / segments as f64;
            points.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    points
}

/// Fills the interior of a counterclockwise-ordered convex polygon with a
/// triangular lattice of points spaced roughly `spacing` apart, staying at
/// least half a spacing away from the boundary so the Delaunay
/// triangulation doesn't produce slivers between interior and boundary
/// vertices.
fn sample_interior_points(boundary_ccw: &[(f64, f64)], spacing: f64) -> Vec<(f64, f64)> {
    let margin = spacing * 0.5;
    let min_x = boundary_ccw
        .iter()
        .map(|p| p.0)
        .fold(f64::INFINITY, f64::min);
    let max_x = boundary_ccw
        .iter()
        .map(|p| p.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = boundary_ccw
        .iter()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min);
    let max_y = boundary_ccw
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max);

    let row_height = spacing * 0.75_f64.sqrt(); // sqrt(3)/2, for a triangular lattice
    let mut points = vec![];
    let mut row = 0i32;
    let mut y = min_y;
    while y <= max_y {
        let x_offset = if row % 2 == 0 { 0.0 } else { spacing * 0.5 };
        let mut x = min_x + x_offset;
        while x <= max_x {
            if inside_with_margin(boundary_ccw, (x, y), margin) {
                points.push((x, y));
            }
            x += spacing;
        }
        row += 1;
        y += row_height;
    }
    points
}

/// True if `p` is inside `boundary_ccw` (a counterclockwise-ordered convex
/// polygon) and at least `margin` away from every edge.
fn inside_with_margin(boundary_ccw: &[(f64, f64)], p: (f64, f64), margin: f64) -> bool {
    let n = boundary_ccw.len();
    for i in 0..n {
        let a = boundary_ccw[i];
        let b = boundary_ccw[(i + 1) % n];
        let edge = (b.0 - a.0, b.1 - a.1);
        let edge_len = (edge.0 * edge.0 + edge.1 * edge.1).sqrt();
        if edge_len < 1e-9 {
            continue;
        }
        let to_point = (p.0 - a.0, p.1 - a.1);
        // Positive for points left of the edge, i.e. the interior side of a
        // counterclockwise polygon.
        let signed_distance = (edge.0 * to_point.1 - edge.1 * to_point.0) / edge_len;
        if signed_distance < margin {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use shalrath::repr::{Brush, BrushPlane, Point, TrianglePlane};

    fn point(x: f64, y: f64, z: f64) -> Point {
        Point {
            x: x as f32,
            y: y as f32,
            z: z as f32,
        }
    }

    // Builds a plane from 3 points, flipping the winding order if needed so the
    // computed normal points toward `centroid` (i.e. inward). This matches the
    // left-handed (UE-space) brushes the functions under test now expect: the
    // .map format winds points so `calculate_normal` is outward in TrenchBroom's
    // right-handed space, and the tb2ue mirror flips that to inward.
    fn plane_inward(v0: Point, v1: Point, v2: Point, centroid: DVec3) -> BrushPlane {
        let normal = calculate_normal(&TrianglePlane { v0, v1, v2 });
        let (v0, v1, v2) = if normal.dot(dvec3(v0) - centroid) > 0.0 {
            (v0, v2, v1)
        } else {
            (v0, v1, v2)
        };
        BrushPlane {
            plane: TrianglePlane { v0, v1, v2 },
            ..Default::default()
        }
    }

    /// The 10x10x10 axis-aligned box brush used throughout these tests, in
    /// left-handed (UE) space, i.e. with inward-wound faces.
    fn box_brush() -> Brush {
        let centroid = DVec3::new(5.0, 5.0, 5.0);
        Brush::new(vec![
            plane_inward(
                point(0.0, 0.0, 0.0),
                point(0.0, 10.0, 0.0),
                point(10.0, 0.0, 0.0),
                centroid,
            ),
            plane_inward(
                point(0.0, 0.0, 10.0),
                point(10.0, 0.0, 10.0),
                point(0.0, 10.0, 10.0),
                centroid,
            ),
            plane_inward(
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(0.0, 0.0, 10.0),
                centroid,
            ),
            plane_inward(
                point(0.0, 10.0, 0.0),
                point(0.0, 10.0, 10.0),
                point(10.0, 10.0, 0.0),
                centroid,
            ),
            plane_inward(
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 10.0),
                point(0.0, 10.0, 0.0),
                centroid,
            ),
            plane_inward(
                point(10.0, 0.0, 0.0),
                point(10.0, 10.0, 0.0),
                point(10.0, 0.0, 10.0),
                centroid,
            ),
        ])
    }

    /// A triangular prism ("wedge"): right-triangle cross-section in xy
    /// extruded along z, uniformly scaled by `s`. Its slanted hypotenuse
    /// face meets everything else at non-right angles.
    fn wedge_brush(s: f64) -> Brush {
        let centroid = DVec3::new(10.0 / 3.0, 10.0 / 3.0, 5.0) * s;
        let a = point(0.0, 0.0, 0.0);
        let b = point(10.0 * s, 0.0, 0.0);
        let c = point(0.0, 10.0 * s, 0.0);
        let a2 = point(0.0, 0.0, 10.0 * s);
        let b2 = point(10.0 * s, 0.0, 10.0 * s);
        let c2 = point(0.0, 10.0 * s, 10.0 * s);
        Brush::new(vec![
            plane_inward(a, b, c, centroid),    // bottom cap
            plane_inward(a2, c2, b2, centroid), // top cap
            plane_inward(a, a2, b2, centroid),  // side face along AB (y = 0)
            plane_inward(a, c, c2, centroid),   // side face along AC (x = 0)
            plane_inward(b, b2, c2, centroid),  // hypotenuse side face along BC
        ])
    }

    #[test]
    fn convert_to_mesh_bakes_outward_facing_normals_without_flipping_winding() {
        // Regression test: with no explicit normals, pseudocooker derives a
        // per-vertex lighting normal from triangle winding -- which is tuned
        // for Unreal's culling/collision convention, not for "true" outward
        // geometric direction, and (since brush_to_mesh welds shared-edge
        // positions across adjacent faces) ends up averaging normals across
        // faces at every corner instead of giving flat per-face shading.
        // convert_to_mesh now bakes an explicit, geometrically-correct
        // outward normal per face via `Corner::with_normal`, independent of
        // triangle winding, so lighting can be fixed without touching
        // winding at all -- and therefore without risking the
        // culling/collision correctness the watertightness tests above
        // already cover.
        let brush = box_brush();
        let (mesh, _origin) = convert_to_mesh(&brush, 0.0, &TextureCollection::new());

        let mesh_centroid = mesh
            .positions
            .iter()
            .fold(DVec3::ZERO, |acc, p| acc + DVec3::new(p[0], p[1], p[2]))
            / mesh.positions.len() as f64;

        for face in &mesh.faces {
            for corner in &face.corners {
                let normal_idx = corner
                    .normal
                    .expect("convert_to_mesh should bake an explicit normal for every corner");
                let n = mesh.normals[normal_idx];
                let n = DVec3::new(n[0], n[1], n[2]);
                let p = mesh.positions[corner.position];
                let p = DVec3::new(p[0], p[1], p[2]);
                assert!(
                    n.dot(p - mesh_centroid) > 0.0,
                    "baked normal {n:?} at vertex {p:?} doesn't point away from the mesh center"
                );
            }
        }
    }

    #[test]
    fn convert_to_mesh_handles_orthogonal_box() {
        // A plain axis-aligned box: every vertex is formed by 3 mutually
        // perpendicular faces, the one case the old angle-based gate handled.
        let brush = box_brush();
        let (mesh, _origin) = convert_to_mesh(&brush, 0.0, &TextureCollection::new());

        // 6 quad faces, triangulated into 2 triangles each (no interior
        // subdivision requested, so this is just the boundary triangulation).
        assert_eq!(mesh.faces.len(), 12);
        for p in &mesh.positions {
            assert!(p.iter().all(|c| c.is_finite()));
        }
    }

    #[test]
    fn convert_to_mesh_returns_deterministic_origin_relative_to_a_brush_corner() {
        let brush = box_brush();

        let (mesh1, origin1) = convert_to_mesh(&brush, 0.0, &TextureCollection::new());
        let (_mesh2, origin2) = convert_to_mesh(&brush, 0.0, &TextureCollection::new());
        assert_eq!(
            origin1, origin2,
            "origin should be deterministic across calls with the same brush"
        );

        // Positions are relative to the origin, so the corner chosen as the
        // origin should itself show up as a mesh vertex at (0, 0, 0).
        assert!(
            mesh1
                .positions
                .iter()
                .any(|p| p[0].abs() < 1e-2 && p[1].abs() < 1e-2 && p[2].abs() < 1e-2),
            "expected a mesh vertex at the origin (0, 0, 0), got {:?}",
            mesh1.positions
        );
    }

    #[test]
    fn convert_to_mesh_handles_non_orthogonal_wedge() {
        // A triangular prism (right triangle cross-section extruded along z).
        // Its two triangular caps meet the two axis-aligned side faces at right
        // angles, but the slanted hypotenuse face meets everything else at
        // non-right angles, and every prior corner of the prism relies on it.
        // The old angle-based gate could only ever find the 2 vertices where
        // the hypotenuse face isn't involved; it would silently drop the other
        // 4 vertices of the shape.
        let brush = wedge_brush(1.0);
        let (mesh, _origin) = convert_to_mesh(&brush, 0.0, &TextureCollection::new());

        // 2 triangular caps (1 tri each) + 3 rectangular sides (2 tris each).
        assert_eq!(mesh.faces.len(), 8);
        for p in &mesh.positions {
            assert!(p.iter().all(|c| c.is_finite()));
        }
    }

    #[test]
    fn convert_to_mesh_subdivides_faces_with_small_spacing() {
        // Same box as above, but with a spacing small enough that each
        // 10x10 face should get extra interior vertices.
        let brush = box_brush();

        let (coarse, _coarse_origin) = convert_to_mesh(&brush, 0.0, &TextureCollection::new());
        let (fine, _fine_origin) = convert_to_mesh(&brush, 3.0, &TextureCollection::new());

        assert!(fine.positions.len() > coarse.positions.len());
        assert!(fine.faces.len() > coarse.faces.len());
        for p in &fine.positions {
            assert!(p.iter().all(|c| c.is_finite()));
        }
    }

    #[test]
    fn triangulate_face_boundary_only_matches_fan_triangle_count() {
        // A convex pentagon with no interior subdivision requested: point
        // count and triangle count should match a plain fan/ear-clip of the
        // boundary (n vertices -> n-2 triangles), just via Delaunay instead.
        let normal = DVec3::Z;
        let boundary = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(5.0, 3.0, 0.0),
            DVec3::new(2.0, 5.0, 0.0),
            DVec3::new(-1.0, 3.0, 0.0),
        ];
        let (points, triangles) = triangulate_face(normal, &boundary, 0.0);

        assert_eq!(points.len(), boundary.len());
        assert_eq!(triangles.len(), boundary.len() - 2);
    }

    #[test]
    fn triangulate_face_adds_interior_points_and_stays_outward_wound() {
        let normal = DVec3::Z;
        let boundary = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(10.0, 10.0, 0.0),
            DVec3::new(0.0, 10.0, 0.0),
        ];
        let (points, triangles) = triangulate_face(normal, &boundary, 2.0);

        // A 10x10 square with 2-unit spacing should pick up interior points.
        assert!(points.len() > boundary.len());
        assert!(!triangles.is_empty());

        for [a, b, c] in triangles {
            let tri_normal = (points[b] - points[a]).cross(points[c] - points[a]);
            assert!(
                tri_normal.dot(normal) > 0.0,
                "triangle wound inward relative to face normal"
            );
        }
    }

    #[test]
    fn subdivide_boundary_adds_evenly_spaced_points_along_each_edge() {
        // A 10x10 square with 3-unit spacing: each 10-unit edge should be
        // split into round(10/3) = 3 segments, i.e. 2 extra points per edge.
        let corners = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let corners_3d: Vec<DVec3> = corners
            .iter()
            .map(|&(x, y)| DVec3::new(x, y, 0.0))
            .collect();
        let subdivided = subdivide_boundary(&corners, &corners_3d, 3.0);

        assert_eq!(subdivided.len(), corners.len() * 3);
        for &(x, y) in &subdivided {
            // Every point should still lie exactly on the square's perimeter.
            assert!((x == 0.0 || x == 10.0 || y == 0.0 || y == 10.0));
            assert!((0.0..=10.0).contains(&x) && (0.0..=10.0).contains(&y));
        }
    }

    #[test]
    fn subdivide_boundary_is_a_no_op_for_non_positive_spacing() {
        let corners = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let corners_3d: Vec<DVec3> = corners
            .iter()
            .map(|&(x, y)| DVec3::new(x, y, 0.0))
            .collect();
        assert_eq!(subdivide_boundary(&corners, &corners_3d, 0.0), corners);
    }

    #[test]
    fn triangulate_face_has_no_long_sliver_edges_along_the_boundary() {
        // Regression test: before boundary edges were subdivided, the outer
        // ring of triangles had one long (unsubdivided) edge on the boundary
        // and much shorter edges connecting to the dense interior lattice,
        // producing thin slivers. With boundary subdivision, no triangle
        // edge should be much longer than the requested spacing.
        let normal = DVec3::Z;
        let boundary = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(20.0, 0.0, 0.0),
            DVec3::new(20.0, 20.0, 0.0),
            DVec3::new(0.0, 20.0, 0.0),
        ];
        let spacing = 3.0;
        let (points, triangles) = triangulate_face(normal, &boundary, spacing);

        let mut max_edge_len: f64 = 0.0;
        for [a, b, c] in &triangles {
            for (i, j) in [(*a, *b), (*b, *c), (*c, *a)] {
                max_edge_len = max_edge_len.max(points[i].distance(points[j]));
            }
        }
        // A generous bound: the longest edge (e.g. a lattice-row diagonal)
        // shouldn't be more than double the requested spacing.
        assert!(
            max_edge_len <= spacing * 2.0,
            "found a sliver-length edge: {max_edge_len}"
        );
    }

    #[test]
    fn weld_vertices_merges_close_points_and_remaps_corners() {
        let mut mesh = pseudocooker::MeshInput {
            // Positions 0 and 2 are float-close but not identical, as two
            // faces' independently-reconstructed copies of a shared corner
            // would be; position 1 is far away and should be untouched.
            positions: vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [1e-4, 1e-4, 0.0]],
            uvs: Vec::new(),
            normals: Vec::new(),
            faces: vec![pseudocooker::Face {
                material_index: 0,
                corners: vec![
                    pseudocooker::Corner::new(0),
                    pseudocooker::Corner::new(1),
                    pseudocooker::Corner::new(2),
                ],
            }],
            material_names: vec![],
        };

        weld_vertices(&mut mesh, WELD_EPSILON);

        assert_eq!(mesh.positions.len(), 2);
        let corner_positions: Vec<usize> =
            mesh.faces[0].corners.iter().map(|c| c.position).collect();
        assert_eq!(
            corner_positions[0], corner_positions[2],
            "the two close vertices should have welded to the same index"
        );
        assert_ne!(corner_positions[0], corner_positions[1]);
    }

    #[test]
    fn convert_to_mesh_has_no_boundary_edges() {
        // Any closed brush mesh should be watertight: every edge shared by
        // exactly two triangles. Checked across a right-angle box, a
        // non-orthogonal wedge, and a subdivided box.
        let box_brush = box_brush();

        let wedge_brush = wedge_brush(1.0);

        for (brush, spacing) in [(&box_brush, 0.0), (&box_brush, 3.0), (&wedge_brush, 0.0)] {
            let (mesh, _origin) = convert_to_mesh(brush, spacing, &TextureCollection::new());
            let boundary_edges = find_boundary_edges(&mesh);
            assert!(
                boundary_edges.is_empty(),
                "found holes/non-manifold edges: {boundary_edges:?}"
            );
        }
    }

    #[test]
    fn convert_to_mesh_has_no_boundary_edges_for_reported_repro_brushes() {
        // Regression test for a user-reported repro: at production spacing
        // (64.0), both brushes here used to leave the watertightness check
        // with a hole. The cause was a spade Delaunay triangulation quirk:
        // boundary subdivision places points exactly on the straight line
        // between the corners they subdivide, and the triangulator isn't
        // numerically stable for such near-collinear input -- it could
        // insert a spurious near-zero-area triangle spanning a corner and
        // one of its own subdivision points, *on top of* an already-correct
        // and gapless triangulation of the rest of the face, leaving that
        // extra triangle's edges unmatched. See `triangulate_face`'s
        // `is_degenerate` filter, which drops such triangles.
        const MAP: &str = r#"
{
"classname" "worldspawn"
{
( 16 0 -96 ) ( 15.666666666666668 1 -95.8095238095238 ) ( 20 0 -93.85714285714286 ) __TB_empty 14.399982 24.005892 108.43492 1.0540923 -7728.329
( 144 96 -16 ) ( 143.66666666666669 97 -15.809523809523817 ) ( 145 96 -15.714285714285715 ) __TB_empty -16 -12.594524 0 0.99999994 1.3557225
( 16 0 -96 ) ( 20 0 -93.85714285714286 ) ( 17 0 -95.7142857142857 ) __TB_empty -7.547184 21.158709 15.9453945 1.0400156 31.327183
( 144 96 -16 ) ( 145 96 -15.714285714285715 ) ( 148 96 -13.857142857142854 ) __TB_empty 17.207546 -9.999427 15.9453945 1.0400156 31.327183
( 16 0 -96 ) ( 17 0 -95.7142857142857 ) ( 15.666666666666668 1 -95.8095238095238 ) __TB_empty -16 -12.594524 0 0.99999994 1.3557225
( 144 96 -16 ) ( 148 96 -13.857142857142854 ) ( 143.66666666666669 97 -15.809523809523817 ) __TB_empty 11.199934 24.017677 108.43492 1.0540923 -7728.329
}
{
( 48 -48 -32 ) ( 53 -50 -31 ) ( 49 -48.333333333333314 -32 ) __TB_empty -8 23.948685 0 1.0000006 467.69745
( 48 -48 -32 ) ( 48 -47 -32 ) ( 53 -50 -31 ) __TB_empty 0 -6.050085 90 1 -45.130215
( 48 -48 -32 ) ( 49 -48.333333333333314 -32 ) ( 48 -47 -32 ) __TB_empty 4.7999954 -0.399621 341.56506 1.0540925 0.9729834
( 224 -16 -16 ) ( 224 -15 -16 ) ( 225 -16.333333333333314 -16 ) __TB_empty -12.800003 -5.5997744 341.56506 1.0540925 0.9729834
( 224 -16 -16 ) ( 229 -18 -15 ) ( 224 -15 -16 ) __TB_empty 0 -3.9229012 90 1 -45.130215
( 224 -16 -16 ) ( 225 -16.333333333333314 -16 ) ( 229 -18 -15 ) __TB_empty -8 -8.051315 0 1.0000006 467.69745
}
}
"#;
        let (_, ast) =
            shalrath::parser::repr::parse_map(MAP.trim()).expect("parse embedded test map");
        for ent in ast.0 {
            for brush in ent.brushes.0.iter() {
                // The embedded map is raw (right-handed) TrenchBroom data, so
                // convert it to left-handed UE space first, as production does.
                let brush = crate::tb2ue::brush(brush.clone());
                let (mesh, _origin) = convert_to_mesh(&brush, 64.0, &TextureCollection::new());
                let boundary_edges = find_boundary_edges(&mesh);
                assert!(
                    boundary_edges.is_empty(),
                    "found holes/non-manifold edges: {boundary_edges:?}"
                );
            }
        }
    }

    #[test]
    fn convert_to_mesh_has_no_boundary_edges_for_clipped_brushes() {
        // Regression test for a user-reported repro (a brush cut with the
        // clip tool): see /workspace/tmp.map.
        const MAP: &str = r#"
{
"classname" "trigger_hazard"
{
( 240 -112 16 ) ( 240 -111 16 ) ( 240 -112 17 ) __TB_empty 0 0 0 1 1
( 240 -112 16 ) ( 241 -112 16 ) ( 240 -111 16 ) __TB_empty 0 0.99999905 0 1 1
( 368 16 48 ) ( 368 17 48 ) ( 369 16 48 ) __TB_empty 0 0.99999905 0 1 1
( 368 16 48 ) ( 369 16 48 ) ( 368 16 49 ) __TB_empty 0 0 0 1 1
( 256 -48 48 ) ( 240 -64 48 ) ( 240 -64 176 ) __TB_empty 0 0 0 1 1
}
{
( 240 -144 16 ) ( 240 -143 16 ) ( 240 -144 17 ) __TB_empty 0 0 0 1 1
( 256 -80 48 ) ( 240 -96 176 ) ( 240 -96 48 ) __TB_empty 0 0 0 1 1
( 240 -144 16 ) ( 240 -144 17 ) ( 241 -144 16 ) __TB_empty 0 0 0 1 1
( 240 -144 16 ) ( 241 -144 16 ) ( 240 -143 16 ) __TB_empty 0 0 0 1 1
( 368 -16 48 ) ( 368 -15 48 ) ( 369 -16 48 ) __TB_empty 0 0 0 1 1
( 368 -16 48 ) ( 369 -16 48 ) ( 368 -16 49 ) __TB_empty 0 0 0 1 1
( 368 -16 48 ) ( 368 -16 49 ) ( 368 -15 48 ) __TB_empty 0 0 0 1 1
}
}
"#;
        let (_, ast) =
            shalrath::parser::repr::parse_map(MAP.trim()).expect("parse embedded test map");
        for ent in ast.0 {
            for (i, brush) in ent.brushes.0.iter().enumerate() {
                // The embedded map is raw (right-handed) TrenchBroom data, so
                // convert it to left-handed UE space first, as production does.
                let brush = crate::tb2ue::brush(brush.clone());
                let (mesh, _origin) = convert_to_mesh(&brush, 64.0, &TextureCollection::new());
                let boundary_edges = find_boundary_edges(&mesh);
                assert!(
                    boundary_edges.is_empty(),
                    "brush {i}: found holes/non-manifold edges: {boundary_edges:?}"
                );
            }
        }
    }

    #[test]
    fn convert_to_mesh_has_no_boundary_edges_at_a_subdivision_rounding_tie() {
        // Regression test: a wedge scaled so its leg edges are exactly 1.5x
        // the production FACE_VERTEX_SPACING (64.0) -- i.e. exactly on the
        // rounding boundary between 1 and 2 subdivision segments. Before
        // segment counts were derived from the shared 3D edge length, the
        // two faces meeting at such an edge could round to a different
        // segment count due to a few ULPs of noise from each face's own 2D
        // reprojection, leaving a hole that welding couldn't fix.
        let s = 9.6_f64; // 10 * 9.6 = 96 = 1.5 * 64
        let wedge_brush = wedge_brush(s);
        let (mesh, _origin) = convert_to_mesh(&wedge_brush, 64.0, &TextureCollection::new());
        let boundary_edges = find_boundary_edges(&mesh);
        assert!(
            boundary_edges.is_empty(),
            "found holes/non-manifold edges: {boundary_edges:?}"
        );
    }

    #[test]
    fn convert_to_mesh_welds_shared_vertices_between_adjacent_faces() {
        // The same box brush as the orthogonal-box test, but checked for
        // near-duplicate (but not identical) positions: before welding,
        // adjacent faces reconstructed their shared edge's vertices
        // independently and landed a few float32-ULPs apart.
        let brush = box_brush();
        let (mesh, _origin) = convert_to_mesh(&brush, 3.0, &TextureCollection::new());

        for i in 0..mesh.positions.len() {
            for j in (i + 1)..mesh.positions.len() {
                let a = mesh.positions[i];
                let b = mesh.positions[j];
                let d =
                    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                assert!(
                    d >= WELD_EPSILON,
                    "found an unwelded near-duplicate vertex pair at distance {d:e}"
                );
            }
        }
    }
}
