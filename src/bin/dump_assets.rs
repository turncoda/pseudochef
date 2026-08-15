use repak::PakBuilder;
use std::fs::{File, create_dir_all};
use std::io::Cursor;
use std::path::Path;
use std::process::exit;
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

fn read_example_pak() {
    const EXAMPLE_PAK: &[u8] = include_bytes!("pakchunk347-Windows.pak");
    let mut reader = Cursor::new(EXAMPLE_PAK);
    let pak = PakBuilder::new().reader(&mut reader).unwrap();
    pak.get("arrow_down.uexp", &mut reader).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        const EXAMPLE_PATH: &str =
            "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Pseudoregalia";
        println!("Usage example: dump_assets.exe \"{}\"", EXAMPLE_PATH);
        exit(1);
    }
    if args[1] == "--download-oodle" {
        // Force repak to download Oodle DLL.
        read_example_pak();
        exit(0);
    }
    let game_dir = Path::new(&args[1]);
    let pak_path = game_dir.join("pseudoregalia/Content/Paks/pseudoregalia-Windows.pak");
    let mut pak_file = File::open(pak_path).expect("failed to open pak file");
    let texture_out_dir = game_dir.join("textures");
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
        let out_path = texture_out_dir.join(bin_path);
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
