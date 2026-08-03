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

pub fn parse_bool(opt: Option<&String>) -> bool {
    let s: &str = opt.map(|s| s.as_ref()).unwrap_or("");
    match s {
        "1" => true,
        "0" => false,
        _ => false,
    }
}

pub fn parse_vec3(opt: Option<&String>) -> DVec3 {
    let s: &str = opt.map(|s| s.as_ref()).unwrap_or("0 0 0");
    tb_vec3_to_ue_dvec3(s)
}

pub fn parse_angle(opt: Option<&String>) -> i16 {
    let s: &str = opt.map(|s| s.as_ref()).unwrap_or("");
    let angle = match s {
        "" => 0,
        _ => s.parse().unwrap(),
    };
    let angle = -angle; // TB (right-handed) to UE (left-handed)
    angle
}

pub fn get_string_with_default<'a>(opt: Option<&'a String>, default: &'a str) -> &'a str {
    opt.map(|s| s.as_ref()).unwrap_or(default)
}
