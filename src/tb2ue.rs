use glam::DVec3;

const TB_TO_UNREAL_SCALE: f32 = 4.0;

pub fn point(mut a: DVec3) -> DVec3 {
    a.y = -a.y;
    a *= TB_TO_UNREAL_SCALE as f64;
    a
}

fn flip_y(point: &mut shalrath::repr::Point) {
    point.y = -point.y;
}

fn scale_point(point: &mut shalrath::repr::Point, scale: f32) {
    point.x *= scale;
    point.y *= scale;
    point.z *= scale;
}

fn scale(brush: &mut shalrath::repr::Brush, scale: f32) {
    for plane in &mut brush.0 {
        scale_point(&mut plane.plane.v0, scale);
        scale_point(&mut plane.plane.v1, scale);
        scale_point(&mut plane.plane.v2, scale);
    }
}

fn mirror_xz(brush: &mut shalrath::repr::Brush) {
    let mut planes = vec![];
    for plane in &mut brush.0 {
        flip_y(&mut plane.plane.v0);
        flip_y(&mut plane.plane.v1);
        flip_y(&mut plane.plane.v2);
        planes.push(plane);
    }
}

pub fn brush(mut b: shalrath::repr::Brush) -> shalrath::repr::Brush {
    mirror_xz(&mut b);
    scale(&mut b, TB_TO_UNREAL_SCALE);
    b
}

pub fn angles(mut a: DVec3) -> DVec3 {
    a *= -1.0;
    a
}
