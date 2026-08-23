use std::{
    collections::{HashMap, HashSet},
    io::{Seek, Write},
};
use unreal_asset::types::PackageIndex;

const KEY_TITLE: &str = "Title_18_8D403C334BBBCC29B73D3CACCDAF0A08";
const KEY_AUTHOR: &str = "Author_6_5E436BFF41A27B8B13653A8CEC5D15A6";
const KEY_LEVEL_NAME: &str = "LevelName_2_392769FD4066EFFA0CC1F99E8D749886";
const KEY_PLAYER_START_TAG: &str = "PlayerStartTag_5_7797C3C742DE3A0B8EEE189EDBEF3683";
const KEY_UPGRADES: &str = "Upgrades_12_339EDA2D4B022358B32C3984E9FAE5F1";
const KEY_LEVEL_DISPLAY_NAMES: &str = "LevelDisplayNames_25_DDBCC3034DAA6A5AE0EB22963E27299A";

pub struct GamePreset {
    pub title: String,
    pub author: String,
    pub level_name: String,
    pub player_start_tag: String,
    pub upgrades: HashSet<String>,
    pub level_display_names: HashMap<String, String>,
}

pub fn pak_add<W: Write + Seek>(pak: &mut repak::PakWriter<W>, preset: &GamePreset) {
    const EXAMPLE_PRESET_UASSET: &[u8] =
        include_bytes!("Mods/PseudoMenuMod/GamePresets/Preset_Example.uasset");
    const EXAMPLE_PRESET_UEXP: &[u8] =
        include_bytes!("Mods/PseudoMenuMod/GamePresets/Preset_Example.uexp");
    let mut uasset = unreal_asset::Asset::new(
        std::io::Cursor::new(EXAMPLE_PRESET_UASSET),
        Some(std::io::Cursor::new(EXAMPLE_PRESET_UEXP)),
        unreal_asset::engine_version::EngineVersion::VER_UE5_1,
        None,
    )
    .unwrap();

    let export = uasset.get_export_mut(PackageIndex::new(1)).unwrap();
    let unreal_asset::Export::DataTableExport(dt_export) = export else {
        panic!();
    };
    set_dt_str_property(dt_export, KEY_TITLE, &preset.title);

    // TODO rename export to match file name

    let mut cooked_uasset_bytes = vec![];
    let mut cooked_uexp_bytes = vec![];
    uasset
        .write_data(
            &mut std::io::Cursor::new(&mut cooked_uasset_bytes),
            Some(&mut std::io::Cursor::new(&mut cooked_uexp_bytes)),
        )
        .unwrap();
    let uasset_path = "Mods/PseudoMenu/GamePresets/Preset_Example.uasset";
    let uexp_path = "Mods/PseudoMenu/GamePresets/Preset_Example.uexp";
    pak.write_file(&uasset_path, true, &cooked_uasset_bytes)
        .unwrap();
    pak.write_file(&uexp_path, true, &cooked_uexp_bytes)
        .unwrap();
}

fn set_dt_str_property(
    export: &mut unreal_asset::exports::DataTableExport<PackageIndex>,
    name: &str,
    value: &str,
) {
    let prop =
        find_dt_str_property_mut(export, name).expect(&format!("couldn't find property: {}", name));
    prop.value = Some(value.to_string());
}

fn find_dt_str_property_mut<'a>(
    export: &'a mut unreal_asset::exports::DataTableExport<PackageIndex>,
    name: &str,
) -> Option<&'a mut unreal_asset::properties::str_property::StrProperty> {
    let mut result = None;
    let props = &mut export.table.data;
    for prop in props {
        for prop in &mut prop.value {
            if let unreal_asset::properties::Property::StrProperty(str_prop) = prop
                && str_prop.name.get_content(|content| content == name)
            {
                result = Some(str_prop);
                break;
            }
        }
    }
    result
}
