use std::collections::HashMap;
use std::collections::HashSet;

use std::io::Seek;
use std::io::Write;

use unreal_asset::Asset;
use unreal_asset::Export;
use unreal_asset::engine_version::EngineVersion;
use unreal_asset::exports::DataTableExport;
use unreal_asset::properties::Property;
use unreal_asset::properties::map_property::MapProperty;
use unreal_asset::properties::str_property::StrProperty;
use unreal_asset::types::FName;
use unreal_asset::types::PackageIndex;

const KEY_TITLE: &str = "Title_18_8D403C334BBBCC29B73D3CACCDAF0A08";
const KEY_AUTHOR: &str = "Author_6_5E436BFF41A27B8B13653A8CEC5D15A6";
const KEY_LEVEL_NAME: &str = "LevelName_2_392769FD4066EFFA0CC1F99E8D749886";
const KEY_PLAYER_START_TAG: &str = "PlayerStartTag_5_7797C3C742DE3A0B8EEE189EDBEF3683";
const KEY_UPGRADES: &str = "Upgrades_12_339EDA2D4B022358B32C3984E9FAE5F1";
#[allow(dead_code)]
const KEY_LEVEL_DISPLAY_NAMES: &str = "LevelDisplayNames_25_DDBCC3034DAA6A5AE0EB22963E27299A";

pub struct GamePreset {
    pub title: String,
    pub author: String,
    pub level_name: String,
    pub player_start_tag: String,
    pub upgrades: HashSet<String>,
    #[allow(dead_code)]
    pub level_display_names: HashMap<String, String>,
}

pub fn pak_add<W: Write + Seek>(pak: &mut repak::PakWriter<W>, preset: &GamePreset) {
    const EXAMPLE_PRESET_UASSET: &[u8] =
        include_bytes!("Mods/PseudoMenuMod/GamePresets/Preset_Example.uasset");
    const EXAMPLE_PRESET_UEXP: &[u8] =
        include_bytes!("Mods/PseudoMenuMod/GamePresets/Preset_Example.uexp");
    let mut uasset = Asset::new(
        std::io::Cursor::new(EXAMPLE_PRESET_UASSET),
        Some(std::io::Cursor::new(EXAMPLE_PRESET_UEXP)),
        EngineVersion::VER_UE5_1,
        None,
    )
    .unwrap();

    let export_name = format!("Preset_{}", preset.level_name);

    // Create FNames upfront to avoid multiple mutable borrow of uasset.
    let upgrade_fnames: Vec<FName> = preset
        .upgrades
        .iter()
        .map(|name| uasset.add_fname(name))
        .collect();
    let export_fname = uasset.add_fname(&export_name);

    let export = uasset.get_export_mut(PackageIndex::new(1)).unwrap();
    let Export::DataTableExport(dt_export) = export else {
        panic!();
    };
    set_dt_str_property(dt_export, KEY_TITLE, &preset.title);
    set_dt_str_property(dt_export, KEY_AUTHOR, &preset.author);
    set_dt_str_property(dt_export, KEY_LEVEL_NAME, &preset.level_name);
    set_dt_str_property(dt_export, KEY_PLAYER_START_TAG, &preset.player_start_tag);
    set_upgrades(dt_export, upgrade_fnames);

    // Rename export to match file name, just to be safe.
    dt_export.normal_export.base_export.object_name = export_fname;

    let mut cooked_uasset_bytes = vec![];
    let mut cooked_uexp_bytes = vec![];
    uasset
        .write_data(
            &mut std::io::Cursor::new(&mut cooked_uasset_bytes),
            Some(&mut std::io::Cursor::new(&mut cooked_uexp_bytes)),
        )
        .unwrap();
    let uasset_path = format!("Mods/PseudoMenu/GamePresets/{}.uasset", export_name);
    let uexp_path = format!("Mods/PseudoMenu/GamePresets/{}.uexp", export_name);
    pak.write_file(&uasset_path, true, &cooked_uasset_bytes)
        .unwrap();
    pak.write_file(&uexp_path, true, &cooked_uexp_bytes)
        .unwrap();
}

fn set_upgrades(dt_export: &mut DataTableExport<PackageIndex>, upgrade_fnames: Vec<FName>) {
    let map_prop = find_dt_map_property_mut(dt_export, KEY_UPGRADES)
        .expect(&format!("couldn't find property: {}", KEY_UPGRADES));
    // TODO in theory, we don't need to get all the keys but I don't know of a way to just get one.
    let keys: Vec<&Property> = map_prop.value.keys().collect();
    let key_base = keys[0].clone();
    let value_base = map_prop.value.get_by_key(&key_base).unwrap().clone();
    while map_prop.value.len() > 0 {
        map_prop.value.remove_by_index(map_prop.value.len() - 1);
    }
    for fname in upgrade_fnames {
        let Property::NameProperty(mut k) = key_base.clone() else {
            panic!();
        };
        let Property::IntProperty(mut v) = value_base.clone() else {
            panic!();
        };
        k.value = fname;
        v.value = 1;
        map_prop.value.insert(k.into(), v.into());
    }
}

fn set_dt_str_property(export: &mut DataTableExport<PackageIndex>, name: &str, value: &str) {
    let prop =
        find_dt_str_property_mut(export, name).expect(&format!("couldn't find property: {}", name));
    prop.value = Some(value.to_string());
}

fn find_dt_map_property_mut<'a>(
    export: &'a mut DataTableExport<PackageIndex>,
    name: &str,
) -> Option<&'a mut MapProperty> {
    let mut result = None;
    let props = &mut export.table.data;
    for prop in props {
        for prop in &mut prop.value {
            if let Property::MapProperty(map_prop) = prop
                && map_prop.name.get_content(|content| content == name)
            {
                result = Some(map_prop);
                break;
            }
        }
    }
    result
}

fn find_dt_str_property_mut<'a>(
    export: &'a mut DataTableExport<PackageIndex>,
    name: &str,
) -> Option<&'a mut StrProperty> {
    let mut result = None;
    let props = &mut export.table.data;
    for prop in props {
        for prop in &mut prop.value {
            if let Property::StrProperty(str_prop) = prop
                && str_prop.name.get_content(|content| content == name)
            {
                result = Some(str_prop);
                break;
            }
        }
    }
    result
}
