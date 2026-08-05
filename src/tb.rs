use glam::DVec3;

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
    let v: Vec<f64> = s.as_ref().split_whitespace().map(|n| n.parse().unwrap()).collect();
    assert_eq!(v.len(), 3);
    DVec3::from_slice(&v)
}

/// Parses a single number as yaw in a DVec3 of pitch, yaw, and roll.
/// e.g. "3" -> DVec3{0, 3, 0}
pub fn unwrap_angle<S: AsRef<str>>(opt: Option<S>) -> DVec3 {
    let Some(s) = opt else {
        return DVec3::default();
    };
    let s = s.as_ref();
    if s.len() == 0 {
        return DVec3::default();
    }
    let opt: Result<f64, _> = s.parse();
    let Ok(angle) = opt else {
        panic!("failed to parse 64-bit float angle from: '{}'", s);
    };
    DVec3::new(0.0, angle, 0.0)
}

pub fn unwrap_string_or<'a, S: AsRef<str>>(opt: Option<&'a S>, default: &'a str) -> &'a str {
    opt.map(|s| s.as_ref()).unwrap_or(default)
}
