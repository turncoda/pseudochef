use crate::{tb, ue};
use glam::{DQuat, DVec3};

const TB_TO_UNREAL_SCALE: f32 = 3.125; // TB 32 => UE 100

pub(crate) fn point(mut a: DVec3) -> DVec3 {
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

pub(crate) fn brush(mut b: shalrath::repr::Brush) -> shalrath::repr::Brush {
    mirror_xz(&mut b);
    scale(&mut b, TB_TO_UNREAL_SCALE);
    b
}

/// Right-handed Eulerian angles, applied in ZXY order.
struct EulerRotZXY {
    pub z: f64,
    pub x: f64,
    pub y: f64,
}

fn euler(pyr: tb::PitchYawRoll) -> EulerRotZXY {
    let tb::PitchYawRoll { pitch, yaw, roll } = pyr;

    // Convert pitch-yaw-roll to TB-space (right-handed, +Z up, +X forward) Euler rotation angles.
    // TB defines pitch as the negative of the right-handed Y-axis rotation angle.
    let x = roll;
    let y = -pitch;
    let z = yaw;

    EulerRotZXY { z, x, y }
}

pub(crate) fn rot(pyr: tb::PitchYawRoll) -> ue::PitchYawRoll {
    let EulerRotZXY { z, x, y } = euler(pyr);

    // Re-order into pitch, yaw, roll (YZX) order because that's how UE stores its angles.
    // ... oh, and UE uses left-hand rule for Z but right-hand rule for X and Y. Because fuck you.
    let pitch = y;
    let yaw = -z;
    let roll = x;

    ue::PitchYawRoll { pitch, yaw, roll }
}

pub(crate) fn quat(pyr: tb::PitchYawRoll) -> DQuat {
    let EulerRotZXY { z, x, y } = euler(pyr);

    // Yaw-roll-pitch (ZXY) was determined empirically to be the rotation order used by UE.
    // Also, the angles are negated to conform to UE's left-handed coordinate system.
    DQuat::from_euler(glam::EulerRot::ZXY, -z, -x, -y)
}
