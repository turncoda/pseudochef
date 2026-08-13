use repak::PakBuilder;
use std::fs::{File, create_dir_all};
use std::io::Cursor;
use std::path::Path;
use unreal_asset::Asset;
use unreal_asset::engine_version::EngineVersion::VER_UE5_1;
use unreal_asset::exports::ExportNormalTrait;
use unreal_asset::types::PackageIndex;

const INCLUDE: &[&str] = &["pseudoregalia/Content/MatTex/Textures"];

const EXCLUDE: &[&str] = &[
    "pseudoregalia/Content/MatTex/Textures/Characters",
    "pseudoregalia/Content/MatTex/Textures/Misc",
    "pseudoregalia/Content/MatTex/Textures/Noise",
    "pseudoregalia/Content/MatTex/Textures/Props",
    "pseudoregalia/Content/MatTex/Textures/UI",
    "pseudoregalia/Content/MatTex/Textures/VFX",
    "pseudoregalia/Content/MatTex/Textures/tex_Lever",
    "pseudoregalia/Content/MatTex/Textures/Baily_Floor", // has a ubulk
    "pseudoregalia/Content/MatTex/Textures/T_GridChecker_A", // has a ubulk
    "pseudoregalia/Content/MatTex/Textures/TilingNoise05", // has odd format
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: dump_textures path/to/my.pak path/to/dump/dir"
    );
    let mut pak_file = File::open(&args[1]).expect("failed to open pak file");
    let out_dir = Path::new(&args[2]);
    let pak = PakBuilder::new()
        .reader(&mut pak_file)
        .expect("failed to read pak file");
    let mut entries = pak.files().clone();
    entries.retain(|path| INCLUDE.iter().any(|include| path.starts_with(include)));
    entries.retain(|path| !EXCLUDE.iter().any(|exclude| path.starts_with(exclude)));
    entries.retain(|path| path.ends_with(".uasset"));
    for entry in entries {
        let uasset_path = Path::new(&entry);
        let uexp_path = uasset_path.with_extension("uexp");
        let bin_path = uasset_path.with_extension("ppm");
        let bin_path = bin_path.strip_prefix("pseudoregalia/Content").unwrap();
        let out_path = out_dir.join(bin_path);
        if let Some(parent) = out_path.parent() {
            create_dir_all(parent).expect("failed to create parent dirs");
        }
        let uasset_bytes = pak
            .get(uasset_path.to_str().unwrap(), &mut pak_file)
            .expect("failed to read uasset file in pak");
        let uexp_bytes = pak
            .get(uexp_path.to_str().unwrap(), &mut pak_file)
            .expect("failed to read uexp file in pak");
        let uasset = Asset::new(
            Cursor::new(uasset_bytes),
            Some(Cursor::new(uexp_bytes)),
            VER_UE5_1,
            None,
        )
        .expect("failed to parse uasset");
        let extra_bytes = &uasset
            .get_export(PackageIndex::new(1))
            .unwrap()
            .get_normal_export()
            .unwrap()
            .extras;
        match pseudotex::decode(&extra_bytes) {
            Ok(img) => {
                let mut out_file = File::create(&out_path).unwrap();
                let bytes_written = img.write_ppm(&mut out_file).unwrap();
                println!(
                    "wrote {} bytes ({}W x {}H) to '{}'",
                    bytes_written,
                    img.width,
                    img.height,
                    out_path.to_string_lossy()
                );
            }
            Err(err) => {
                println!("failed to dump '{}': {}", out_path.to_string_lossy(), err);
            }
        }
    }
}
