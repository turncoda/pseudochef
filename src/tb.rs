use glam::DVec3;

/// In relation to right-handed Eulerian angles, TB defines its rotations as follows:
///
///     Pitch = -Y
///     Yaw   =  Z
///     Roll  =  X
///
#[derive(Default)]
pub struct PitchYawRoll {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

/// e.g. "1" -> true
pub fn unwrap_bool<S: AsRef<str>>(opt: Option<S>) -> bool {
    let Some(s) = opt else {
        return false;
    };
    match s.as_ref() {
        "1" => true,
        "0" => false,
        _ => false,
    }
}

/// e.g. "0 0 0" -> DVec3{0, 0, 0}
pub fn unwrap_vec3<S: AsRef<str>>(opt: Option<S>) -> DVec3 {
    let Some(s) = opt else {
        return DVec3::default();
    };
    let v: Vec<f64> = s
        .as_ref()
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(v.len(), 3);
    DVec3::from_slice(&v)
}

pub fn unwrap_rot<S: AsRef<str>>(opt: Option<S>) -> PitchYawRoll {
    let Some(s) = opt else {
        return PitchYawRoll::default();
    };
    let v: Vec<f64> = s
        .as_ref()
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(v.len(), 3);
    PitchYawRoll {
        pitch: v[0],
        yaw: v[1],
        roll: v[2],
    }
}

/// Parses a single number as yaw.
pub fn unwrap_yaw<S: AsRef<str>>(opt: Option<S>) -> PitchYawRoll {
    let Some(s) = opt else {
        return PitchYawRoll::default();
    };
    let s = s.as_ref();
    if s.len() == 0 {
        return PitchYawRoll::default();
    }
    let opt: Result<f64, _> = s.parse();
    let Ok(angle) = opt else {
        panic!("failed to parse 64-bit float angle from: '{}'", s);
    };
    PitchYawRoll {
        pitch: 0.0,
        yaw: angle,
        roll: 0.0,
    }
}

pub fn unwrap_string_or<'a, S: AsRef<str>>(opt: Option<&'a S>, default: &'a str) -> &'a str {
    opt.map(|s| s.as_ref()).unwrap_or(default)
}
