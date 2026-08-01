pub fn parse_bool(opt: Option<&String>) -> bool {
    let s: &str = opt.map(|s| s.as_ref()).unwrap_or("");
    match s {
        "1" => true,
        "0" => false,
        _ => false,
    }
}
