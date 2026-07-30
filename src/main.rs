use glam::DVec3;
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
use brush_to_mesh::{AxisAlignedBoundingBox, convert_to_mesh, get_aabb, tb_space_to_ue_space};

mod unreal_asset_ext;
use unreal_asset_ext::{deep_clone_export, deep_delete_export};

// For debugging purposes; may not be called
#[allow(dead_code)]
mod obj_export;

const MISE_FILES: &[(&str, &[u8])] = &[
    ("BP_Hazard.uasset", include_bytes!("mise/BP_Hazard.uasset")),
    ("BP_Hazard.uexp", include_bytes!("mise/BP_Hazard.uexp")),
    (
        "BP_SafeZone.uasset",
        include_bytes!("mise/BP_SafeZone.uasset"),
    ),
    ("BP_SafeZone.uexp", include_bytes!("mise/BP_SafeZone.uexp")),
    (
        "M_SafeZone_Inst.uasset",
        include_bytes!("mise/M_SafeZone_Inst.uasset"),
    ),
    (
        "M_SafeZone_Inst.uexp",
        include_bytes!("mise/M_SafeZone_Inst.uexp"),
    ),
    (
        "M_SafeZone.uasset",
        include_bytes!("mise/M_SafeZone.uasset"),
    ),
    ("M_SafeZone.uexp", include_bytes!("mise/M_SafeZone.uexp")),
    ("M_HazMat.uasset", include_bytes!("mise/M_HazMat.uasset")),
    ("M_HazMat.uexp", include_bytes!("mise/M_HazMat.uexp")),
    (
        "SM_ExampleBox.uasset",
        include_bytes!("mise/SM_ExampleBox.uasset"),
    ),
    (
        "SM_ExampleBox.uexp",
        include_bytes!("mise/SM_ExampleBox.uexp"),
    ),
    (
        "SM_ExampleBox.ubulk",
        include_bytes!("mise/SM_ExampleBox.ubulk"),
    ),
];

const MISE_UMAP: &[u8] = include_bytes!("mise/mise.umap");
const MISE_UEXP: &[u8] = include_bytes!("mise/mise.uexp");

// World-space (map-unit) spacing between generated interior face vertices;
// see `brush_to_mesh::convert_to_mesh`. Smaller values give smoother
// per-vertex lighting at the cost of more geometry.
const FACE_VERTEX_SPACING: f64 = 64.0;

type UnrealExportConstraint<'a, C> = Box<
    dyn Fn(&unreal_asset::Asset<C>, &unreal_asset::exports::NormalExport<PackageIndex>) -> bool
        + 'a,
>;

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
    let idx1 = find_import(umap, "Package", "/Game/Mods/Maps/mise/SM_ExampleBox").unwrap();
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

fn add_material_import<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    path: &str,
) -> PackageIndex {
    let path = format!("/{path}");
    let last_slash_idx = path
        .rfind('/')
        .unwrap_or_else(|| panic!("invalid input to add_static_mesh_import: \"{}\"", path));
    // Hardcode to find M64_CheckerTile as reference
    let idx1 = find_import(
        umap,
        "Package",
        "/Game/MatTex/Materials/Env/Floor/M64_CheckerTile",
    )
    .unwrap();
    let idx2 = find_import(umap, "Material", "M64_CheckerTile").unwrap();

    // Clone 'Package' import (contains actual absolute path to asset in pak)
    let mut import1c = umap.get_import(idx1).unwrap().clone();
    import1c.object_name = umap.add_fname(&path);
    let idx1c = umap.add_import(import1c);

    // Clone 'Material' import, which should reference 'Package' import
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

fn add_jump_bubble<C: Read + Seek>(umap: &mut unreal_asset::Asset<C>, location: DVec3) {
    let idx = find_export(umap, &[with_name("BP_JumpBubble_C")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let root = get_linked_export_mut(umap, idx, "RootComponent").unwrap();
    set_location(root, location);
}

fn add_player_start<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    location: DVec3,
    yaw: i16,
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
    set_yaw(root, yaw);

    idx
}

fn add_safe_zone_actor<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    origin: DVec3,
    extents: DVec3,
) {
    let idx = find_export(umap, &[with_name("BP_SafeZone_C")]).unwrap();
    let idx = deep_clone_export(umap, idx);
    add_actor_to_level(umap, idx);

    let box_collider = get_linked_export_mut(umap, idx, "Box").unwrap();
    set_extents(box_collider, extents);

    let root = get_linked_export_mut(umap, idx, "DefaultSceneRoot").unwrap();
    set_location(root, origin);
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
) {
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
    prop.value = idx;
}

fn set_yaw(export: &mut unreal_asset::Export<PackageIndex>, yaw: i16) {
    let prop = find_rot_property_mut(export, "RelativeRotation")
        .expect("couldn't find RelativeRotation property");
    prop.value.y.0 = yaw as f64;
    // For some reason, unreal_asset's RotatorProperty uses Y for yaw, whereas Unreal Engine seems
    // to use Z for yaw, or at least it looks that way in the editor.
}

fn set_extents(export: &mut unreal_asset::Export<PackageIndex>, location: DVec3) {
    let prop =
        find_vec_property_mut(export, "BoxExtent").expect("couldn't find BoxExtent property");
    prop.value.x.0 = location.x;
    prop.value.y.0 = location.y;
    prop.value.z.0 = location.z;
}

fn set_location(export: &mut unreal_asset::Export<PackageIndex>, location: DVec3) {
    let prop = find_vec_property_mut(export, "RelativeLocation")
        .expect("couldn't find RelativeLocation property");
    prop.value.x.0 = location.x;
    prop.value.y.0 = location.y;
    prop.value.z.0 = location.z;
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
) -> &mut unreal_asset::Export<PackageIndex> {
    let idx2 = find_original_static_mesh_actor(umap);
    let idx3 = deep_clone_export(umap, idx2);
    add_actor_to_level(umap, idx3);

    // Find the cloned StaticMeshComponent0 and redirect it to the new import.
    let export4 = get_linked_export_mut(umap, idx3, "StaticMeshComponent").unwrap();
    set_obj_property(export4, "StaticMesh", import_idx);
    set_location(export4, origin);
    export4
}

fn tb_vec3_to_ue_dvec3(s: &str) -> DVec3 {
    let v: Vec<f64> = s.split_whitespace().map(|n| n.parse().unwrap()).collect();
    let dv = DVec3::from_slice(&v);
    tb_space_to_ue_space(dv)
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
    let mut player_starts = HashMap::<String, PackageIndex>::new();
    let mut save_points = Vec::<(PackageIndex, String)>::new();
    let start = Instant::now();
    for ent in ast.0 {
        let props: HashMap<&str, &str> = ent
            .properties
            .0
            .iter()
            .map(|p| (p.key.as_ref(), p.value.as_ref()))
            .collect();
        let Some(&classname) = props.get("classname") else {
            continue;
        };
        match classname {
            "worldspawn" => {
                for brush in &ent.brushes.0 {
                    num_world_brushes += 1;
                    let name = format!("WorldBrush{}", num_world_brushes);
                    let (abs_path, origin) =
                        pak_add_brush(&mut pak, brush, &map_name, &name).unwrap();
                    let mesh = add_static_mesh_import(&mut umap, &abs_path);
                    // with the cooking code as is we can't use multiple materials until we sort faces by material
                    let mat = add_material_import(&mut umap, &brush.0[0].texture);
                    let comp = add_static_mesh_actor(&mut umap, mesh, origin);
                    let props = &mut comp.get_normal_export_mut().unwrap().properties;
                    for prop in props {
                        let unreal_asset::properties::Property::ArrayProperty(arr) = prop else {
                            continue;
                        };
                        let unreal_asset::properties::Property::ObjectProperty(obj) =
                            &mut arr.value[0]
                        else {
                            continue;
                        };
                        obj.value = mat;
                    }
                }
            }
            "trigger_safe_zone" => {
                for brush in &ent.brushes.0 {
                    let AxisAlignedBoundingBox { origin, extents } = get_aabb(brush);
                    add_safe_zone_actor(&mut umap, origin, extents);
                }
            }
            "trigger_hazard_zone" => {
                for brush in &ent.brushes.0 {
                    num_hazard_brushes += 1;
                    let name = format!("HazardZoneBrush{}", num_hazard_brushes);
                    let (abs_path, origin) =
                        pak_add_brush(&mut pak, brush, &map_name, &name).unwrap();
                    let idx = add_static_mesh_import(&mut umap, &abs_path);
                    add_hazard_actor(&mut umap, idx, origin);
                }
            }
            "info_player_start" => {
                let origin = tb_vec3_to_ue_dvec3(props.get("origin").unwrap_or(&"0 0 0"));
                let angle: i16 = props.get("angle").map(|s| s.parse().unwrap()).unwrap_or(0);
                let angle = -angle; // TB (right-handed) to UE (left-handed)
                let tag = props.get("tag").unwrap_or(&"gameStart");
                let idx = add_player_start(&mut umap, origin, angle, tag);
                player_starts.insert(tag.to_string(), idx);
            }
            "misc_jump_bubble" => {
                let origin = tb_vec3_to_ue_dvec3(props.get("origin").unwrap_or(&"0 0 0"));
                add_jump_bubble(&mut umap, origin);
            }
            "misc_save_point" => {
                let origin = tb_vec3_to_ue_dvec3(props.get("origin").unwrap_or(&"0 0 0"));
                let target = props.get("target").unwrap_or(&"gameStart").to_string();
                let idx = add_save_point(&mut umap, origin);
                save_points.push((idx, target));
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

    // rename level export (for swag only; seemingly inconsequential)
    {
        let fname = umap.add_fname(&map_name);
        let idx = find_export(&umap, &[with_name("mise")]).expect("couldn't find mise");
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
            find_export(&umap, &[with_name("PlayerStart")]).unwrap(),
            find_original_static_mesh_actor(&umap),
        ];

        for idx in idxs {
            deep_delete_export(&mut umap, idx);
        }
    }

    let mut final_umap = std::io::Cursor::new(vec![]);
    let mut final_uexp = std::io::Cursor::new(vec![]);
    umap.write_data(&mut final_umap, Some(&mut final_uexp))
        .expect("failed to serialize umap");

    std::fs::write("dbg.umap", final_umap.get_ref()).unwrap();
    std::fs::write("dbg.uexp", final_uexp.get_ref()).unwrap();

    // TODO also rename mise export
    let umap_path = format!("Mods/Maps/{}.umap", map_name);
    let uexp_path = format!("Mods/Maps/{}.uexp", map_name);
    pak.write_file(&umap_path, true, final_umap.get_ref())
        .expect("failed to write umap to pak");
    pak.write_file(&uexp_path, true, final_uexp.get_ref())
        .expect("failed to write uexp to pak");

    for (name, bytes) in MISE_FILES {
        let path = format!("Mods/Maps/mise/{}", name);
        pak.write_file(&path, true, bytes)
            .unwrap_or_else(|_| panic!("failed to write {} to pak", name));
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
