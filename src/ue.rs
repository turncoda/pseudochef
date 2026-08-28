/// In relation to right-handed Eulerian angles, UE defines its rotations as follows:
///
///     Pitch =  Y
///     Yaw   = -Z
///     Roll  =  X
///
#[derive(Default)]
pub(crate) struct PitchYawRoll {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

impl From<glam::DQuat> for PitchYawRoll {
    fn from(quat: glam::DQuat) -> Self {
        let (z, x, y) = quat.to_euler(glam::EulerRot::ZXY);

        const RAD_TO_DEG: f64 = 180.0 / std::f64::consts::PI;
        let x = x * RAD_TO_DEG;
        let y = y * RAD_TO_DEG;
        let z = z * RAD_TO_DEG;

        let pitch = -y;
        let yaw = z;
        let roll = -x;

        Self { pitch, yaw, roll }
    }
}
