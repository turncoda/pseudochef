use glam::{DQuat, DVec3};
use shalrath::parser::repr::parse_map;
use std::collections::HashMap;
use std::fs::{File, read_to_string};
use std::io::{Read, Seek, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use unreal_asset::exports::ExportBaseTrait;
use unreal_asset::exports::ExportNormalTrait;
use unreal_asset::types::PackageIndex;

mod brush_to_mesh;
use brush_to_mesh::{AxisAlignedBoundingBox, convert_to_mesh, get_aabb};

mod unreal_asset_ext;
use unreal_asset_ext::{deep_clone_export, deep_delete_export};

mod upgrade_data;
use upgrade_data::{get_upgrade_data, set_upgrade_data};

mod tb;
mod tb2ue;
mod ue;

// For debugging purposes; may not be called
#[allow(dead_code)]
mod obj_export;

const MISE_FILES: &[(&str, &[u8], &[u8], &[u8])] = &[
    (
        "SM_StarterGate",
        include_bytes!("base/ModPack_Base/SM_StarterGate.uasset"),
        include_bytes!("base/ModPack_Base/SM_StarterGate.uexp"),
        include_bytes!("base/ModPack_Base/SM_StarterGate.ubulk"),
    ),
    (
        "MI_ExampleMat",
        include_bytes!("base/ModPack_Base/MI_ExampleMat.uasset"),
        include_bytes!("base/ModPack_Base/MI_ExampleMat.uexp"),
        &[],
    ),
    (
        "MI_StarterGate",
        include_bytes!("base/ModPack_Base/MI_StarterGate.uasset"),
        include_bytes!("base/ModPack_Base/MI_StarterGate.uexp"),
        &[],
    ),
    (
        "T_StarterGate",
        include_bytes!("base/ModPack_Base/T_StarterGate.uasset"),
        include_bytes!("base/ModPack_Base/T_StarterGate.uexp"),
        &[],
    ),
    (
        "T_Debug",
        include_bytes!("base/ModPack_Base/T_Debug.uasset"),
        include_bytes!("base/ModPack_Base/T_Debug.uexp"),
        &[],
    ),
    (
        "BP_Hazard",
        include_bytes!("base/ModPack_Base/BP_Hazard.uasset"),
        include_bytes!("base/ModPack_Base/BP_Hazard.uexp"),
        &[],
    ),
    (
        "BP_SafeZone",
        include_bytes!("base/ModPack_Base/BP_SafeZone.uasset"),
        include_bytes!("base/ModPack_Base/BP_SafeZone.uexp"),
        &[],
    ),
    (
        "M_SafeZone_Inst",
        include_bytes!("base/ModPack_Base/M_SafeZone_Inst.uasset"),
        include_bytes!("base/ModPack_Base/M_SafeZone_Inst.uexp"),
        &[],
    ),
    (
        "M_SafeZone",
        include_bytes!("base/ModPack_Base/M_SafeZone.uasset"),
        include_bytes!("base/ModPack_Base/M_SafeZone.uexp"),
        &[],
    ),
    (
        "M_HazMat",
        include_bytes!("base/ModPack_Base/M_HazMat.uasset"),
        include_bytes!("base/ModPack_Base/M_HazMat.uexp"),
        &[],
    ),
    (
        "SM_ExampleBox",
        include_bytes!("base/ModPack_Base/SM_ExampleBox.uasset"),
        include_bytes!("base/ModPack_Base/SM_ExampleBox.uexp"),
        include_bytes!("base/ModPack_Base/SM_ExampleBox.ubulk"),
    ),
];

const MISE_UMAP: &[u8] = include_bytes!("base/ModPack_Base.umap");
const MISE_UEXP: &[u8] = include_bytes!("base/ModPack_Base.uexp");

// Triangulation density control. Higher = more sparse. UE units.
const FACE_VERTEX_SPACING: f64 = 256.0;

type UnrealExportConstraint<'a, C> = Box<
    dyn Fn(&unreal_asset::Asset<C>, &unreal_asset::exports::NormalExport<PackageIndex>) -> bool
        + 'a,
>;

struct Marker {
    origin: DVec3,
    quat: DQuat,
}

fn with_import<'a, C: Read + Seek>(
    obj_prop_name: &'a str,
    import_name: &'a str,
) -> UnrealExportConstraint<'a, C> {
    Box::new(move |asset, export| {
        let mut matching_prop = None;
        for prop in &export.properties {
            if let unreal_asset::properties::Property::ObjectProperty(obj_prop) = prop
                && obj_prop
                    .name
                    .get_content(|content| content == obj_prop_name)
            {
                matching_prop = Some(obj_prop);
            }
        }

        let Some(prop) = matching_prop else {
            return false;
        };

        // this is expected to be an import
        assert!(prop.value.index < 0);
        let import = asset
            .get_import(prop.value)
            .unwrap_or_else(|| panic!("failed to get import {}", prop.value.index));
        import
            .object_name
            .get_content(|content| content == import_name)
    })
}

fn with_name<'a, C: Read + Seek>(name: &'a str) -> UnrealExportConstraint<'a, C> {
    Box::new(move |_, export| {
        export
            .base_export
            .object_name
            .get_content(|content| content == name)
    })
}

fn find_export<C: Read + Seek>(
    asset: &unreal_asset::Asset<C>,
    constraints: &[UnrealExportConstraint<C>],
) -> Option<PackageIndex> {
    let mut maybe_idx = None;
    for (i, export) in asset.asset_data.exports.iter().enumerate() {
        if let Some(normal_export) = export.get_normal_export()
            && constraints.iter().all(|f| f(asset, normal_export))
        {
            maybe_idx = Some(PackageIndex::new((i + 1) as i32));
        }
    }
    maybe_idx
}

fn find_vec_property<'a>(
    export: &'a unreal_asset::Export<PackageIndex>,
    name: &str,
) -> Option<&'a unreal_asset::properties::vector_property::VectorProperty> {
    let mut result = None;
    let props = &export.get_normal_export().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::StructProperty(struct_prop) = prop
            && struct_prop.name.get_content(|content| content == name)
        {
            for prop in &struct_prop.value {
                if let unreal_asset::properties::Property::VectorProperty(vec_prop) = prop {
                    result = Some(vec_prop);
                }
            }
        }
    }
    result
}

fn find_vec_property_mut<'a>(
    export: &'a mut unreal_asset::Export<PackageIndex>,
    name: &str,
) -> Option<&'a mut unreal_asset::properties::vector_property::VectorProperty> {
    let mut result = None;
    let props = &mut export.get_normal_export_mut().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::StructProperty(struct_prop) = prop
            && struct_prop.name.get_content(|content| content == name)
        {
            for prop in &mut struct_prop.value {
                if let unreal_asset::properties::Property::VectorProperty(vec_prop) = prop {
                    result = Some(vec_prop);
                }
            }
        }
    }
    result
}

fn find_rot_property<'a>(
    export: &'a unreal_asset::Export<PackageIndex>,
    name: &str,
) -> Option<&'a unreal_asset::properties::vector_property::RotatorProperty> {
    let mut result = None;
    let props = &export.get_normal_export().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::StructProperty(struct_prop) = prop
            && struct_prop.name.get_content(|content| content == name)
        {
            for prop in &struct_prop.value {
                if let unreal_asset::properties::Property::RotatorProperty(rot_prop) = prop {
                    result = Some(rot_prop);
                }
            }
        }
    }
    result
}

fn find_rot_property_mut<'a>(
    export: &'a mut unreal_asset::Export<PackageIndex>,
    name: &str,
) -> Option<&'a mut unreal_asset::properties::vector_property::RotatorProperty> {
    let mut result = None;
    let props = &mut export.get_normal_export_mut().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::StructProperty(struct_prop) = prop
            && struct_prop.name.get_content(|content| content == name)
        {
            for prop in &mut struct_prop.value {
                if let unreal_asset::properties::Property::RotatorProperty(rot_prop) = prop {
                    result = Some(rot_prop);
                }
            }
        }
    }
    result
}

fn find_obj_property<'a>(
    export: &'a unreal_asset::Export<PackageIndex>,
    name: &str,
) -> Option<&'a unreal_asset::properties::object_property::ObjectProperty> {
    let mut result = None;
    let props = &export.get_normal_export().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::ObjectProperty(obj_prop) = prop
            && obj_prop.name.get_content(|content| content == name)
        {
            result = Some(obj_prop);
        }
    }
    result
}

fn find_bool_property_mut<'a>(
    export: &'a mut unreal_asset::Export<PackageIndex>,
    key: &str,
) -> Option<&'a mut unreal_asset::properties::int_property::BoolProperty> {
    let mut result = None;
    let props = &mut export.get_normal_export_mut().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::BoolProperty(bool_prop) = prop
            && bool_prop.name.get_content(|content| content == key)
        {
            result = Some(bool_prop);
        }
    }
    result
}

fn find_text_property_mut<'a>(
    export: &'a mut unreal_asset::Export<PackageIndex>,
    key: &str,
) -> Option<&'a mut unreal_asset::properties::str_property::TextProperty> {
    let mut result = None;
    let props = &mut export.get_normal_export_mut().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::TextProperty(text_prop) = prop
            && text_prop.name.get_content(|content| content == key)
        {
            result = Some(text_prop);
        }
    }
    result
}

fn find_name_property_mut<'a>(
    export: &'a mut unreal_asset::Export<PackageIndex>,
    key: &str,
) -> Option<&'a mut unreal_asset::properties::str_property::NameProperty> {
    let mut result = None;
    let props = &mut export.get_normal_export_mut().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::NameProperty(name_prop) = prop
            && name_prop.name.get_content(|content| content == key)
        {
            result = Some(name_prop);
        }
    }
    result
}

fn find_array_property_mut<'a>(
    export: &'a mut unreal_asset::Export<PackageIndex>,
    name: &str,
) -> Option<&'a mut unreal_asset::properties::array_property::ArrayProperty> {
    let mut result = None;
    let props = &mut export.get_normal_export_mut().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::ArrayProperty(arr_prop) = prop
            && arr_prop.name.get_content(|content| content == name)
        {
            result = Some(arr_prop);
        }
    }
    result
}

fn find_obj_property_mut<'a>(
    export: &'a mut unreal_asset::Export<PackageIndex>,
    name: &str,
) -> Option<&'a mut unreal_asset::properties::object_property::ObjectProperty> {
    let mut result = None;
    let props = &mut export.get_normal_export_mut().unwrap().properties;
    for prop in props {
        if let unreal_asset::properties::Property::ObjectProperty(obj_prop) = prop
            && obj_prop.name.get_content(|content| content == name)
        {
            result = Some(obj_prop);
        }
    }
    result
}

fn find_import<C: Read + Seek>(
    asset: &mut unreal_asset::Asset<C>,
    class_name: &str,
    object_name: &str,
) -> Option<PackageIndex> {
    for (i, import) in asset.imports.iter().enumerate() {
        if import
            .class_name
            .get_content(|content| content == class_name)
            && import
                .object_name
                .get_content(|content| content == object_name)
        {
            return Some(PackageIndex::new(-((i + 1) as i32)));
        }
    }
    None
}

fn add_actor_to_level<C: Read + Seek>(asset: &mut unreal_asset::Asset<C>, idx: PackageIndex) {
    let level_idx = find_export(asset, &[with_name("PersistentLevel")]).unwrap();
    let export = asset.get_export_mut(level_idx).unwrap();
    export
        .get_base_export_mut()
        .create_before_serialization_dependencies
        .push(idx);
    if let unreal_asset::Export::LevelExport(level_export) = export {
        level_export.actors.push(idx);
    } else {
        panic!();
    }
}

fn pak_add_brush<W: Write + Seek>(
    pak: &mut repak::PakWriter<W>,
    brush: &shalrath::repr::Brush,
    level_name: &str,
    asset_name: &str,
) -> Option<(String, DVec3)> {
    let (mesh, origin) = convert_to_mesh(brush, FACE_VERTEX_SPACING);
    let cooked = pseudocooker::cook(&mesh, asset_name);
    let uasset_path = format!("Mods/Maps/{}/gen/{}.uasset", level_name, asset_name);
    pak.write_file(&uasset_path, true, &cooked.uasset)
        .expect("failed to write uasset to pak");
    let uexp_path = format!("Mods/Maps/{}/gen/{}.uexp", level_name, asset_name);
    pak.write_file(&uexp_path, true, &cooked.uexp)
        .expect("failed to write uexp to pak");
    let abs_path_no_ext = format!("/Game/Mods/Maps/{}/gen/{}", level_name, asset_name);
    Some((abs_path_no_ext, origin))
}

fn add_static_mesh_import<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    path: &str,
) -> PackageIndex {
    let last_slash_idx = path
        .rfind('/')
        .unwrap_or_else(|| panic!("invalid input to add_static_mesh_import: \"{}\"", path));
    // Hardcode to find SM_ExampleBox and use it as the reference import.
    let idx1 = find_import(
        umap,
        "Package",
        "/Game/Mods/Maps/ModPack_Base/SM_ExampleBox",
    )
    .unwrap();
    let idx2 = find_import(umap, "StaticMesh", "SM_ExampleBox").unwrap();

    // Clone 'Package' import (contains actual absolute path to asset in pak)
    let mut import1c = umap.get_import(idx1).unwrap().clone();
    import1c.object_name = umap.add_fname(path);
    let idx1c = umap.add_import(import1c);

    // Clone 'StaticMesh' import, which should reference 'Package' import
    let mut import2c = umap.get_import(idx2).unwrap().clone();
    let basename = &path[last_slash_idx + 1..];
    import2c.object_name = umap.add_fname(basename);
    import2c.outer_index = idx1c;

    // Return the index of the newly-added import.
    umap.add_import(import2c)
}

fn add_save_point<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    location: DVec3,
) -> PackageIndex {
    let idx = find_export(umap, &[with_name("BP_SavePoint_C")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let root = get_linked_export_mut(umap, idx, "RootComponent").unwrap();
    set_location(root, location);

    idx
}

fn add_parent<C: Read + Seek>(umap: &mut unreal_asset::Asset<C>, location: DVec3) -> PackageIndex {
    let idx = find_export(umap, &[with_name("BP_SwA_StaticMesh_C")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let root = get_linked_export_mut(umap, idx, "RootComponent").unwrap();
    set_location(root, location);

    idx
}

fn add_switch<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    location: DVec3,
    rot: ue::PitchYawRoll,
) -> PackageIndex {
    let idx = find_export(umap, &[with_name("BP_HitSwitch_C")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let root = get_linked_export_mut(umap, idx, "RootComponent").unwrap();
    set_location(root, location);
    set_rotation(root, rot);

    idx
}

fn add_gate<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    location: DVec3,
    rot: ue::PitchYawRoll,
) -> PackageIndex {
    let idx = find_gate_static_mesh_actor(umap);
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let root = get_linked_export_mut(umap, idx, "RootComponent").unwrap();
    set_location(root, location);
    set_rotation(root, rot);

    idx
}

fn add_jump_bubble<C: Read + Seek>(umap: &mut unreal_asset::Asset<C>, location: DVec3) {
    let idx = find_export(umap, &[with_name("BP_JumpBubble_C")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let root = get_linked_export_mut(umap, idx, "RootComponent").unwrap();
    set_location(root, location);
}

fn add_upgrade<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    location: DVec3,
    upgrade_name: &str,
    quick_pickup: bool,
) -> PackageIndex {
    let idx = find_export(umap, &[with_name("BP_UpgradeBase_C")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let export = umap.get_export_mut(idx).unwrap();
    set_text_property(export, "rowName", upgrade_name);
    set_bool_property(export, "debugQuickPickup", quick_pickup);

    {
        let mut _upgrade_data = get_upgrade_data(upgrade_name);
        // TODO override here
        set_upgrade_data(export, _upgrade_data);
    }

    let root = get_linked_export_mut(umap, idx, "RootComponent").unwrap();
    set_location(root, location);

    idx
}

fn add_player_start<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    location: DVec3,
    rot: ue::PitchYawRoll,
    tag: &str,
) -> PackageIndex {
    let idx = find_export(umap, &[with_name("PlayerStart")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let tag = umap.add_fname(tag);
    let export = umap.get_export_mut(idx).unwrap();
    set_name_property(export, "PlayerStartTag", tag);

    let root = get_linked_export_mut(umap, idx, "RootComponent").unwrap();
    set_location(root, location);
    set_rotation(root, rot);

    idx
}

fn add_safe_zone_actor<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    origin: DVec3,
    extents: DVec3,
) -> PackageIndex {
    let idx = find_export(umap, &[with_name("BP_SafeZone_C")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let box_collider = get_linked_export_mut(umap, idx, "Box").unwrap();
    set_extents(box_collider, extents);

    let root = get_linked_export_mut(umap, idx, "DefaultSceneRoot").unwrap();
    set_location(root, origin);

    idx
}

fn add_hazard_actor<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    import_idx: PackageIndex,
    origin: DVec3,
) {
    // Hardcoded to find the BP_Hazard_C and use it as the reference export.
    let idx = find_export(umap, &[with_name("BP_Hazard_C")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let export_sm = get_linked_export_mut(umap, idx, "StaticMesh").unwrap();
    set_obj_property(export_sm, "StaticMesh", import_idx);

    let export_root = get_linked_export_mut(umap, idx, "DefaultSceneRoot").unwrap();
    set_location(export_root, origin);
}

/// Get a reference to the export referenced by the ObjectProperty with name |object_name| on the
/// export at index |idx|.
fn get_linked_export<'a, C: Read + Seek>(
    umap: &'a unreal_asset::Asset<C>,
    idx: PackageIndex,
    object_name: &str,
) -> Option<&'a unreal_asset::Export<PackageIndex>> {
    let export = umap.get_export(idx)?;
    let prop = find_obj_property(export, object_name)?;
    assert!(prop.value.index > 0); // must be export
    umap.get_export(prop.value)
}

/// Get a mutable reference to the export referenced by the ObjectProperty with name |object_name|
/// on the export at index |idx|.
fn get_linked_export_mut<'a, C: Read + Seek>(
    umap: &'a mut unreal_asset::Asset<C>,
    idx: PackageIndex,
    object_name: &str,
) -> Option<&'a mut unreal_asset::Export<PackageIndex>> {
    let export = umap.get_export(idx)?;
    let prop = find_obj_property(export, object_name)?;
    assert!(prop.value.index > 0); // must be export
    umap.get_export_mut(prop.value)
}

fn set_bool_property(export: &mut unreal_asset::Export<PackageIndex>, key: &str, value: bool) {
    let prop = find_bool_property_mut(export, key).unwrap();
    prop.value = value;
}

fn set_text_property(export: &mut unreal_asset::Export<PackageIndex>, key: &str, value: &str) {
    let prop = find_text_property_mut(export, key).unwrap();
    prop.culture_invariant_string = Some(value.to_string());
}

fn set_name_property(
    export: &mut unreal_asset::Export<PackageIndex>,
    key: &str,
    value: unreal_asset::types::FName,
) {
    let prop = find_name_property_mut(export, key).unwrap();
    prop.value = value;
}

fn set_obj_property(
    export: &mut unreal_asset::Export<PackageIndex>,
    object_name: &str,
    idx: PackageIndex,
) -> PackageIndex {
    let base_export = export.get_base_export_mut();
    let export_name = base_export.object_name.get_owned_content();
    if idx.index != 0 {
        base_export
            .create_before_serialization_dependencies
            .push(idx);
    }
    // this error message should really be in the function.
    let prop = find_obj_property_mut(export, object_name).unwrap_or_else(|| {
        panic!(
            "couldn't find object property \"{}\" in {}",
            object_name, export_name
        )
    });
    let old_idx = prop.value;
    prop.value = idx;
    old_idx
}

fn set_extents(export: &mut unreal_asset::Export<PackageIndex>, location: DVec3) {
    let prop =
        find_vec_property_mut(export, "BoxExtent").expect("couldn't find BoxExtent property");
    prop.value.x.0 = location.x;
    prop.value.y.0 = location.y;
    prop.value.z.0 = location.z;
}

fn set_rot_property(
    export: &mut unreal_asset::Export<PackageIndex>,
    name: &str,
    rot: ue::PitchYawRoll,
) {
    let prop =
        find_rot_property_mut(export, name).expect(&format!("couldn't find property: {}", name));
    prop.value.x.0 = rot.pitch;
    prop.value.y.0 = rot.yaw;
    prop.value.z.0 = rot.roll;
}

fn get_rot_property(export: &unreal_asset::Export<PackageIndex>, name: &str) -> ue::PitchYawRoll {
    let prop = find_rot_property(export, name).expect(&format!("couldn't find property: {}", name));
    ue::PitchYawRoll {
        pitch: prop.value.x.0,
        yaw: prop.value.y.0,
        roll: prop.value.z.0,
    }
}

fn set_vec_property(export: &mut unreal_asset::Export<PackageIndex>, name: &str, value: DVec3) {
    let prop =
        find_vec_property_mut(export, name).expect(&format!("couldn't find property: {}", name));
    prop.value.x.0 = value.x;
    prop.value.y.0 = value.y;
    prop.value.z.0 = value.z;
}

fn get_vec_property(export: &unreal_asset::Export<PackageIndex>, name: &str) -> DVec3 {
    let prop = find_vec_property(export, name).expect(&format!("couldn't find property: {}", name));
    DVec3::new(prop.value.x.0, prop.value.y.0, prop.value.z.0)
}

fn set_location(export: &mut unreal_asset::Export<PackageIndex>, location: DVec3) {
    set_vec_property(export, "RelativeLocation", location);
}

fn get_location(export: &unreal_asset::Export<PackageIndex>) -> DVec3 {
    get_vec_property(export, "RelativeLocation")
}

fn set_rotation(export: &mut unreal_asset::Export<PackageIndex>, rot: ue::PitchYawRoll) {
    set_rot_property(export, "RelativeRotation", rot);
}

fn get_rotation(export: &unreal_asset::Export<PackageIndex>) -> ue::PitchYawRoll {
    get_rot_property(export, "RelativeRotation")
}

fn find_gate_static_mesh_actor<C: Read + Seek>(umap: &unreal_asset::Asset<C>) -> PackageIndex {
    let idx = find_export(
        umap,
        &[
            with_name("StaticMeshComponent0"),
            with_import("StaticMesh", "SM_StarterGate"),
        ],
    )
    .unwrap();
    let export = umap.get_export(idx).unwrap();

    // Return the parent (a StaticMeshActor).
    export.get_base_export().outer_index
}

// Finds the reference StaticMeshActor by its component's name and mesh.
// Clones made by `add_static_mesh_actor` never match: `deep_clone_export`
// renames the cloned component and `set_obj_property` repoints its
// StaticMesh at the generated mesh's import, so this stays unambiguous no
// matter how many clones have been added.
fn find_original_static_mesh_actor<C: Read + Seek>(umap: &unreal_asset::Asset<C>) -> PackageIndex {
    let idx = find_export(
        umap,
        &[
            with_name("StaticMeshComponent0"),
            with_import("StaticMesh", "SM_ExampleBox"),
        ],
    )
    .unwrap();
    let export = umap.get_export(idx).unwrap();

    // Return the parent (a StaticMeshActor).
    export.get_base_export().outer_index
}

fn add_static_mesh_actor<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    import_idx: PackageIndex,
    origin: DVec3,
) {
    let idx2 = find_original_static_mesh_actor(umap);
    let idx3 = deep_clone_export(umap, idx2);
    add_actor_to_level(umap, idx3);

    // Find the cloned StaticMeshComponent0 and redirect it to the new import.
    let export4 = get_linked_export_mut(umap, idx3, "StaticMeshComponent").unwrap();
    set_obj_property(export4, "StaticMesh", import_idx);
    set_location(export4, origin);
}

fn reroute_imports<C: Read + Seek>(umap: &mut unreal_asset::Asset<C>, src: &str, dst: &str) {
    // Collect indices of all Package imports.
    let mut package_imports_to_rewrite = vec![];
    for (i, import) in umap.imports.iter_mut().enumerate() {
        if import
            .class_name
            .get_content(|content| content != "Package")
        {
            continue;
        }
        if import
            .object_name
            .get_content(|content| content.contains(src))
        {
            package_imports_to_rewrite.push((i, import.object_name.get_owned_content()));
        }
    }

    // Rewrite imports to point to bundled assets, part 2:
    // Replace the mod pack name with the level name.
    for (i, orig_path) in package_imports_to_rewrite {
        let patched_path = orig_path.replacen(src, dst, 1);
        let fname = umap.add_fname(&patched_path);
        let import = &mut umap.imports[i];
        import.object_name = fname;
        println!(
            "Rewrote import {}: '{}' -> '{}'",
            -((i + 1) as i32),
            orig_path,
            patched_path
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    assert_eq!(args.len(), 3, "usage: pseudochef IN_MAP OUT_PAK");

    let in_path = Path::new(&args[1]);
    let map_name = in_path.file_stem().unwrap().to_string_lossy();
    let map_contents = read_to_string(&args[1]).expect("failed to read map file");

    let (_, ast) = parse_map(&map_contents).expect("failed to parse map file");

    let mut umap = unreal_asset::Asset::new(
        std::io::Cursor::new(MISE_UMAP),
        Some(std::io::Cursor::new(MISE_UEXP)),
        unreal_asset::engine_version::EngineVersion::VER_UE5_1,
        None,
    )
    .expect("failed to parse umap");

    let mut pak = repak::PakBuilder::new()
        .compression(vec![repak::Compression::Zlib])
        .writer(
            File::create(&args[2]).expect("failed to open pak file for writing"),
            repak::Version::V11,
            "../../../pseudoregalia/Content/".to_string(),
            None,
        );

    let mut num_world_brushes = 0;
    let mut num_hazard_brushes = 0;

    let mut parent_to_child: Vec<(PackageIndex, String)> = Vec::new();
    let mut switches: Vec<(PackageIndex, String)> = Vec::new();
    let mut save_points: Vec<(PackageIndex, String)> = Vec::new();
    let mut parent_to_marker: Vec<(PackageIndex, String)> = Vec::new();
    // TODO remove Option<>
    let mut safe_zones: Vec<(PackageIndex, Option<String>)> = Vec::new();

    let mut markers: HashMap<String, Marker> = HashMap::new();
    let mut respawn_anchors: HashMap<String, DVec3> = HashMap::new();
    let mut parents: HashMap<String, PackageIndex> = HashMap::new();
    let mut player_starts: HashMap<String, PackageIndex> = HashMap::new();
    let mut tagged_static_mesh_actors: HashMap<String, PackageIndex> = HashMap::new();

    let start = Instant::now();
    for ent in ast.0 {
        let props: HashMap<String, String> = ent
            .properties
            .0
            .into_iter()
            .map(|p| (p.key, p.value))
            .collect();
        let Some(classname) = props.get("classname") else {
            continue;
        };
        match classname.as_ref() {
            "worldspawn" => {
                for brush in ent.brushes.0.into_iter() {
                    let brush = tb2ue::brush(brush);
                    num_world_brushes += 1;
                    let name = format!("WorldBrush{}", num_world_brushes);
                    let (abs_path, origin) =
                        pak_add_brush(&mut pak, &brush, &map_name, &name).unwrap();
                    let idx = add_static_mesh_import(&mut umap, &abs_path);
                    add_static_mesh_actor(&mut umap, idx, origin);
                }
            }
            "trigger_safe_zone" => {
                for brush in ent.brushes.0.into_iter() {
                    let brush = tb2ue::brush(brush);
                    // TODO this should really get the min bounding cuboid, not the AABB. In other
                    // words, allow the bounding box to rotate, since UE box colliders are allowed
                    // to rotate. Otherwise this results in surprising behavior if the user assumes
                    // they can rotate the safe zone bounding box (they can, but the AABB will just
                    // add more - potentially undesirable - collidable volume).
                    let AxisAlignedBoundingBox { origin, extents } = get_aabb(&brush);
                    let idx = add_safe_zone_actor(&mut umap, origin, extents);
                    safe_zones.push((idx, props.get("target").map(|s| s.to_string())));
                }
            }
            "trigger_hazard_zone" => {
                for brush in ent.brushes.0.into_iter() {
                    let brush = tb2ue::brush(brush);
                    num_hazard_brushes += 1;
                    let name = format!("HazardZoneBrush{}", num_hazard_brushes);
                    let (abs_path, origin) =
                        pak_add_brush(&mut pak, &brush, &map_name, &name).unwrap();
                    let idx = add_static_mesh_import(&mut umap, &abs_path);
                    add_hazard_actor(&mut umap, idx, origin);
                }
            }
            "info_respawn_anchor" => {
                let origin = tb2ue::point(tb::unwrap_vec3(props.get("origin")));
                if let Some(tag) = props.get("tag") {
                    respawn_anchors.insert(tag.to_string(), origin);
                }
            }
            "info_player_start" => {
                let origin = tb2ue::point(tb::unwrap_vec3(props.get("origin")));
                let rot = tb2ue::rot(tb::unwrap_yaw(props.get("angle")));
                let tag = tb::unwrap_string_or(props.get("tag"), "gameStart");
                let idx = add_player_start(&mut umap, origin, rot, tag);
                player_starts.insert(tag.to_string(), idx);
            }
            "item_upgrade" => {
                let origin = tb2ue::point(tb::unwrap_vec3(props.get("origin")));
                let upgrade_name = tb::unwrap_string_or(props.get("upgrade"), "attack");
                let quick_pickup = tb::unwrap_bool(props.get("quick_pickup"));
                add_upgrade(&mut umap, origin, upgrade_name, quick_pickup);
            }
            "misc_jump_bubble" => {
                let origin = tb2ue::point(tb::unwrap_vec3(props.get("origin")));
                add_jump_bubble(&mut umap, origin);
            }
            "misc_save_point" => {
                let origin = tb2ue::point(tb::unwrap_vec3(props.get("origin")));
                let target = tb::unwrap_string_or(props.get("target"), "gameStart");
                let idx = add_save_point(&mut umap, origin);
                save_points.push((idx, target.to_string()));
            }
            "prop_gate" => {
                let origin = tb2ue::point(tb::unwrap_vec3(props.get("origin")));
                let rot = tb2ue::rot(tb::unwrap_rot(props.get("angles")));
                let idx = add_gate(&mut umap, origin, rot);
                if let Some(tag) = props.get("tag") {
                    tagged_static_mesh_actors.insert(tag.to_string(), idx);
                }
            }
            "info_parent" => {
                let origin = tb2ue::point(tb::unwrap_vec3(props.get("origin")));
                let idx = add_parent(&mut umap, origin);
                if let Some(child) = props.get("child") {
                    parent_to_child.push((idx, child.clone()));
                }
                if let Some(dst) = props.get("destination") {
                    parent_to_marker.push((idx, dst.clone()));
                }
                if let Some(tag) = props.get("tag") {
                    parents.insert(tag.clone(), idx);
                }
            }
            "info_marker" => {
                let origin = tb2ue::point(tb::unwrap_vec3(props.get("origin")));
                let quat = tb2ue::quat(tb::unwrap_rot(props.get("angles")));
                if let Some(tag) = props.get("tag") {
                    markers.insert(tag.clone(), Marker { origin, quat });
                }
            }
            "func_switch" => {
                let origin = tb2ue::point(tb::unwrap_vec3(props.get("origin")));
                let rot = tb2ue::rot(tb::unwrap_rot(props.get("angles")));
                let idx = add_switch(&mut umap, origin, rot);
                if let Some(target) = props.get("target") {
                    switches.push((idx, target.clone()));
                }
            }
            _ => {}
        };
    }
    let elapsed = start.elapsed();
    println!("World brushes: {}", num_world_brushes);
    println!("Hazard zone brushes: {}", num_hazard_brushes);
    println!(
        "Generated level in {}.",
        humantime::format_duration(Duration::from_millis(elapsed.as_millis() as u64))
    );

    println!("Linking actors...");

    // Link save points to player starts
    for (save_idx, save_target) in save_points {
        let save_export = umap.get_export_mut(save_idx).unwrap();
        let start_idx = if let Some(idx) = player_starts.get(&save_target) {
            println!(
                "save_point @{} -> player_start @{} ({})",
                save_idx, idx.index, save_target
            );
            *idx
        } else {
            println!(
                "Warning: save_point @{} could not find player_start with tag: {}",
                save_idx, save_target
            );
            PackageIndex::new(0)
        };
        set_obj_property(save_export, "associatedPlayerStart", start_idx);
    }

    // Link safe zones to respawn anchors
    for (idx, maybe_target) in safe_zones {
        let root = get_linked_export(&umap, idx, "RootComponent").unwrap();
        let safe_zone_location = get_location(&root);

        let offset = if let Some(target) = maybe_target
            && let Some(anchor_location) = respawn_anchors.get(&target)
        {
            anchor_location - safe_zone_location
        } else {
            DVec3::default()
        };
        println!(
            "safe_zone @{}: respawn_anchor w/ relative offset {}",
            idx, offset
        );

        set_vec_property(umap.get_export_mut(idx).unwrap(), "offset", offset);
    }

    // Link parents to childs
    for (parent_idx, child_tag) in parent_to_child {
        let parent_root = get_linked_export(&umap, parent_idx, "RootComponent").unwrap();
        let parent_location = get_location(&parent_root);

        let Some(child_idx) = tagged_static_mesh_actors.get(&child_tag) else {
            println!("WARNING: couldn't find child with tag: {}", child_tag);
            continue;
        };

        let child_root = get_linked_export(&umap, *child_idx, "RootComponent").unwrap();
        let child_location = get_location(&child_root);
        let child_rotation = get_rotation(&child_root);

        let relative_location = child_location - parent_location;

        let donor_static_mesh_component =
            get_linked_export_mut(&mut umap, *child_idx, "StaticMeshComponent").unwrap();
        let donor_static_mesh_import_idx = set_obj_property(
            donor_static_mesh_component,
            "StaticMesh",
            PackageIndex::new(0),
        );
        let donor_override_materials =
            find_array_property_mut(donor_static_mesh_component, "OverrideMaterials")
                .map(|v| v.clone());
        // TODO double check I did this right
        deep_delete_export(&mut umap, *child_idx);

        let recipient_static_mesh_component =
            get_linked_export_mut(&mut umap, parent_idx, "StaticMesh").unwrap();
        set_obj_property(
            recipient_static_mesh_component,
            "StaticMesh",
            donor_static_mesh_import_idx,
        );
        set_location(recipient_static_mesh_component, relative_location);
        set_rotation(recipient_static_mesh_component, child_rotation);
        let recipient_override_materials =
            find_array_property_mut(recipient_static_mesh_component, "OverrideMaterials");
        if let Some(recipient_override_materials) = recipient_override_materials
            && let Some(donor_override_materials) = donor_override_materials
        {
            recipient_override_materials.value = donor_override_materials.value;
        }
    }

    // Link parents to markers
    for (parent_idx, marker_tag) in parent_to_marker {
        let parent_root = get_linked_export(&umap, parent_idx, "RootComponent").unwrap();
        let parent_location = get_location(&parent_root);

        let Some(marker) = markers.get(&marker_tag) else {
            println!("WARNING: couldn't find marker with tag: {}", marker_tag);
            continue;
        };

        let relative_location = marker.origin - parent_location;

        let parent_export = umap.get_export_mut(parent_idx).unwrap();
        let state_positions = find_array_property_mut(parent_export, "statePositions").unwrap();
        // TODO this is very hardcoded and not flexible (doesn't support multiple positions, assumes
        // there is a second state position in the template actor) and this deep nesting sucks and
        // the error handling sucks
        if let unreal_asset::properties::Property::StructProperty(struct_prop) =
            &mut state_positions.value[1]
        {
            for prop in &mut struct_prop.value {
                if let unreal_asset::properties::Property::StructProperty(struct_prop) = prop {
                    // TODO set rotation as well
                    if struct_prop
                        .name
                        .get_content(|content| content == "Rotation")
                    {
                        for prop in &mut struct_prop.value {
                            if let unreal_asset::properties::Property::QuatProperty(quat_prop) =
                                prop
                            {
                                quat_prop.value.x.0 = marker.quat.x;
                                quat_prop.value.y.0 = marker.quat.y;
                                quat_prop.value.z.0 = marker.quat.z;
                                quat_prop.value.w.0 = marker.quat.w;
                            }
                        }
                    }
                    if struct_prop
                        .name
                        .get_content(|content| content == "Translation")
                    {
                        for prop in &mut struct_prop.value {
                            if let unreal_asset::properties::Property::VectorProperty(vec_prop) =
                                prop
                            {
                                vec_prop.value.x.0 = relative_location.x;
                                vec_prop.value.y.0 = relative_location.y;
                                vec_prop.value.z.0 = relative_location.z;
                            }
                        }
                    }
                }
            }
        }
    }

    // Link switches to parents
    for (switch_idx, parent_tag) in switches {
        let Some(parent_idx) = parents.get(&parent_tag) else {
            println!("WARNING: couldn't find parent with tag: {}", parent_tag);
            continue;
        };
        println!("func_switch @{} -> info_parent @{}", switch_idx, parent_idx);

        let switch_export = umap.get_export_mut(switch_idx).unwrap();
        let associated_actors = find_array_property_mut(switch_export, "associatedActors").unwrap();
        let mut prop = associated_actors.value[0].clone();
        if let unreal_asset::properties::Property::ObjectProperty(obj_prop) = &mut prop {
            obj_prop.value = *parent_idx;
        }
        associated_actors.value.clear();
        associated_actors.value.push(prop);
        switch_export
            .get_base_export_mut()
            .create_before_serialization_dependencies
            .push(*parent_idx);
    }

    // rename level export (for swag only; seemingly inconsequential)
    {
        let fname = umap.add_fname(&map_name);
        let idx = find_export(&umap, &[with_name("ModPack_Base")])
            .expect("couldn't find export: ModPack_Base");
        let export = umap.get_export_mut(idx).unwrap();
        export.get_base_export_mut().object_name = fname;
    }

    // Remove reference actors.
    {
        let idxs = vec![
            find_export(&umap, &[with_name("BP_Hazard_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_SafeZone_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_SavePoint_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_JumpBubble_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_UpgradeBase_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_NPC_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_HitSwitch_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_SwA_StaticMesh_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_ClimbPole_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_BounceHitter_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_TrialGate_C")]).unwrap(),
            find_export(&umap, &[with_name("BP_TimeTrial_C")]).unwrap(),
            find_export(&umap, &[with_name("PlayerStart")]).unwrap(),
            find_original_static_mesh_actor(&umap),
            find_gate_static_mesh_actor(&umap),
        ];

        for idx in idxs {
            deep_delete_export(&mut umap, idx);
        }
    }

    // Design decision: any assets from mod packs that are used in this map are to be bundled with
    // the map rather than be left referencing the mod pack itself, lest we open the door to broken
    // references, mod pack version mismatch, etc. In other words, we're opting for static linking,
    // not dynamic linking. Of course, this unfortunately means larger map pak sizes!

    // Reroute imports to point to bundled assets.
    reroute_imports(&mut umap, "ModPack_Base", &map_name);

    let mut final_umap = std::io::Cursor::new(vec![]);
    let mut final_uexp = std::io::Cursor::new(vec![]);
    umap.write_data(&mut final_umap, Some(&mut final_uexp))
        .expect("failed to serialize umap");

    #[cfg(debug_assertions)]
    {
        std::fs::write("dbg.umap", final_umap.get_ref()).unwrap();
        std::fs::write("dbg.uexp", final_uexp.get_ref()).unwrap();
    }

    let umap_path = format!("Mods/Maps/{}.umap", map_name);
    let uexp_path = format!("Mods/Maps/{}.uexp", map_name);
    pak.write_file(&umap_path, true, final_umap.get_ref())
        .expect("failed to write umap to pak");
    pak.write_file(&uexp_path, true, final_uexp.get_ref())
        .expect("failed to write uexp to pak");

    for (name, uasset_bytes, uexp_bytes, ubulk_bytes) in MISE_FILES {
        println!("Rerouting imports in: {}", name);
        let mut uasset = unreal_asset::Asset::new(
            std::io::Cursor::new(uasset_bytes),
            Some(std::io::Cursor::new(uexp_bytes)),
            unreal_asset::engine_version::EngineVersion::VER_UE5_1,
            None,
        )
        .expect("failed to parse uasset");

        reroute_imports(&mut uasset, "ModPack_Base", &map_name);

        let mut uasset_bytes = vec![];
        let mut uexp_bytes = vec![];
        uasset
            .write_data(
                &mut std::io::Cursor::new(&mut uasset_bytes),
                Some(&mut std::io::Cursor::new(&mut uexp_bytes)),
            )
            .unwrap();

        #[cfg(debug_assertions)]
        {
            std::fs::create_dir_all("dbg").unwrap();
            std::fs::write(&format!("dbg/{}.uasset", name), &uasset_bytes).unwrap();
            std::fs::write(&format!("dbg/{}.uexp", name), &uexp_bytes).unwrap();
        }

        let uasset_path = format!("Mods/Maps/{}/{}.uasset", map_name, name);
        let uexp_path = format!("Mods/Maps/{}/{}.uexp", map_name, name);
        pak.write_file(&uasset_path, true, uasset_bytes)
            .unwrap_or_else(|_| panic!("failed to write: {}", uasset_path));
        pak.write_file(&uexp_path, true, uexp_bytes)
            .unwrap_or_else(|_| panic!("failed to write: {}", uexp_path));
        if ubulk_bytes.len() > 0 {
            let ubulk_path = format!("Mods/Maps/{}/{}.ubulk", map_name, name);
            pak.write_file(&ubulk_path, true, ubulk_bytes)
                .unwrap_or_else(|_| panic!("failed to write: {}", ubulk_path));
        }
    }

    let mut writer = pak.write_index().expect("failed to write pak file");
    let bytes_written = writer
        .stream_position()
        .expect("failed to seek in pak file");
    println!(
        "wrote {} to \"{}\"",
        humansize::format_size(bytes_written, humansize::DECIMAL),
        args[2]
    );
}
