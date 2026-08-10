use glam::{DVec3, DQuat};

const TB_TO_UNREAL_SCALE: f32 = 3.125; // TB 32 => UE 100

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
    // From experiments: when going from TB to UE,
    // pitch and yaw are negated, while roll remains the same.
    // I'm not sure of the mathematical reasoning here.
    a.x *= -1.0; // pitch
    a.y *= -1.0; // yaw
    a

    // To go from (pitch, yaw, roll) to true right-handed Euler rotation, pitch should be inverted.
}

pub fn quat(a: DVec3) -> DQuat {
    // UE is left-handed with the +Z-axis pointing up and +X pointing
    // forward. Therefore:
    //
    //     Roll        = X
    //     Pitch       = Y
    //     Yaw         = Z
    //
    let (pitch, yaw, roll) = a.into();
    // The following angle negations were determined empirically.
    let x = -roll;
    let y = pitch;
    let z = -yaw;
    // The following rotation order was determined empirically.
    DQuat::from_euler(glam::EulerRot::ZXY, z, x, y)
}
