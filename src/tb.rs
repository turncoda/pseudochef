use glam::DVec3;

const TB_TO_UNREAL_SCALE: f32 = 4.0;

fn tb_space_to_ue_space(mut a: DVec3) -> DVec3 {
    a.y = -a.y;
    a *= TB_TO_UNREAL_SCALE as f64;
    a
}

fn tb_vec3_to_ue_dvec3(s: &str) -> DVec3 {
    let v: Vec<f64> = s.split_whitespace().map(|n| n.parse().unwrap()).collect();
    let dv = DVec3::from_slice(&v);
    tb_space_to_ue_space(dv)
}

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

pub fn unwrap_vec3<S: AsRef<str>>(opt: Option<S>) -> DVec3 {
    let Some(s) = opt else {
        return DVec3::default();
    };
    tb_vec3_to_ue_dvec3(s.as_ref())
}

pub fn unwrap_i16<S: AsRef<str>>(opt: Option<S>) -> i16 {
    let Some(s) = opt else {
        return 0;
    };
    let s = s.as_ref();
    if s.len() == 0 {
        return 0;
    }
    let opt: Result<i16, _> = s.parse();
    let Ok(angle) = opt else {
        panic!("failed to parse 16-bit signed integer angle from: '{}'", s);
    };
    let angle = -angle; // TB (right-handed) to UE (left-handed)
    angle
}

pub fn unwrap_string_or<'a, S: AsRef<str>>(opt: Option<&'a S>, default: &'a str) -> &'a str {
    opt.map(|s| s.as_ref()).unwrap_or(default)
}
