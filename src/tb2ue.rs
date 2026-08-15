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
        if let shalrath::repr::TextureOffset::Valve{ u, v } = &mut plane.texture_offset {
            u.x /= scale;
            u.y /= scale;
            u.z /= scale;
            u.d /= scale;

            v.x /= scale;
            v.y /= scale;
            v.z /= scale;
            v.d /= scale;
        }
    }
}

fn mirror_xz(brush: &mut shalrath::repr::Brush) {
    for plane in &mut brush.0 {
        flip_y(&mut plane.plane.v0);
        flip_y(&mut plane.plane.v1);
        flip_y(&mut plane.plane.v2);
        if let shalrath::repr::TextureOffset::Valve{ u, v } = &mut plane.texture_offset {
            u.y = -u.y;
            v.y = -v.y;
        }
    }
}

pub(crate) fn brush(mut b: shalrath::repr::Brush) -> shalrath::repr::Brush {
    mirror_xz(&mut b);
    scale(&mut b, TB_TO_UNREAL_SCALE);
    b
}

/// Right-handed Eulerian angles, in degrees, applied in ZXY order.
struct EulerDegZXY {
    pub z: f64,
    pub x: f64,
    pub y: f64,
}

fn euler(pyr: tb::PitchYawRoll) -> EulerDegZXY {
    let tb::PitchYawRoll { pitch, yaw, roll } = pyr;

    // Convert pitch-yaw-roll to TB-space (right-handed, +Z up, +X forward) Euler rotation angles.
    // TB defines pitch as the negative of the right-handed Y-axis rotation angle.
    let x = roll;
    let y = -pitch;
    let z = yaw;

    EulerDegZXY { z, x, y }
}

pub(crate) fn rot(pyr: tb::PitchYawRoll) -> ue::PitchYawRoll {
    let EulerDegZXY { z, x, y } = euler(pyr);

    // Re-order into pitch, yaw, roll (YZX) order because that's how UE stores its angles.
    // ... oh, and UE uses left-hand rule for Z but right-hand rule for X and Y. Because fuck you.
    let pitch = y;
    let yaw = -z;
    let roll = x;

    ue::PitchYawRoll { pitch, yaw, roll }
}

pub(crate) fn quat(pyr: tb::PitchYawRoll) -> DQuat {
    let EulerDegZXY { z, x, y } = euler(pyr);

    // Negate angles to conform to UE's left-handed coordinate system.
    let x = -x;
    let y = -y;
    let z = -z;

    // Convert to radians in preparation to convert to glam::DQuat.
    const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;
    let x = x * DEG_TO_RAD;
    let y = y * DEG_TO_RAD;
    let z = z * DEG_TO_RAD;

    // Yaw-roll-pitch (ZXY) was determined empirically to be the rotation order used by UE.
    DQuat::from_euler(glam::EulerRot::ZXY, z, x, y)
}
