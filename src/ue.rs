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
