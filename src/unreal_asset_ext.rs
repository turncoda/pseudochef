//! Extensions to `unreal_asset`: deep clone/delete exports.
//! Largely written using Claude Code with access to Unreal Engine 5.1 source code.

use crate::{find_export, find_import, with_name};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek};
use std::sync::atomic::{AtomicI32, Ordering};
use unreal_asset::exports::ExportBaseTrait;
use unreal_asset::exports::ExportNormalTrait;
use unreal_asset::reader::ArchiveTrait;
use unreal_asset::types::PackageIndex;

fn export_clone_counter() -> i32 {
    static COUNT: AtomicI32 = AtomicI32::new(100);
    COUNT.fetch_add(1, Ordering::Relaxed)
}

fn clone_export<C: Read + Seek>(
    asset: &mut unreal_asset::Asset<C>,
    idx: PackageIndex,
) -> Option<PackageIndex> {
    let mut export = asset.get_export(idx)?.clone();
    let old_name = export.get_base_export().object_name.get_owned_content();
    let new_name = format!("pseudochef_{}", old_name);
    let new_fname = asset.add_fname_with_number(&new_name, export_clone_counter());
    export.get_base_export_mut().object_name = new_fname;
    asset.asset_data.exports.push(export);
    Some(PackageIndex::new(asset.asset_data.exports.len() as i32))
}

// Collects `idx` together with every export it owns, directly or transitively
// (i.e. every export whose outer index chain leads back to `idx`). This mirrors
// Unreal's subobject model, where e.g. an actor's components are separate exports
// outered to the actor, and is exactly the set of exports that must be duplicated
// together to deep clone `idx` as a self-contained subtree.
pub(crate) fn collect_owned_exports<C: Read + Seek>(
    asset: &unreal_asset::Asset<C>,
    idx: PackageIndex,
) -> Vec<PackageIndex> {
    let mut owned = vec![idx];
    let mut i = 0;
    while i < owned.len() {
        let outer = owned[i];
        for j in 0..asset.asset_data.exports.len() {
            let candidate = PackageIndex::new((j + 1) as i32);
            if owned.contains(&candidate) {
                continue;
            }
            if asset
                .get_export(candidate)
                .unwrap()
                .get_base_export()
                .outer_index
                == outer
            {
                owned.push(candidate);
            }
        }
        i += 1;
    }
    owned
}

fn remap_package_index(index: &mut PackageIndex, old_to_new: &HashMap<PackageIndex, PackageIndex>) {
    if let Some(&new_index) = old_to_new.get(index) {
        *index = new_index;
    }
}

// Rewrites any PackageIndex found in `prop` (recursing into arrays, sets, maps and
// structs) according to `old_to_new`. References outside `old_to_new` (e.g. imports,
// or exports outside the cloned subtree) are left untouched.
pub(crate) fn remap_property(
    prop: &mut unreal_asset::properties::Property,
    old_to_new: &HashMap<PackageIndex, PackageIndex>,
) {
    use unreal_asset::properties::Property;
    match prop {
        Property::ObjectProperty(p) => remap_package_index(&mut p.value, old_to_new),
        Property::DelegateProperty(p) => remap_package_index(&mut p.value.object, old_to_new),
        Property::MulticastDelegateProperty(p) => {
            for delegate in &mut p.value {
                remap_package_index(&mut delegate.object, old_to_new);
            }
        }
        Property::MulticastSparseDelegateProperty(p) => {
            for delegate in &mut p.value {
                remap_package_index(&mut delegate.object, old_to_new);
            }
        }
        Property::MulticastInlineDelegateProperty(p) => {
            for delegate in &mut p.value {
                remap_package_index(&mut delegate.object, old_to_new);
            }
        }
        Property::ArrayProperty(p) => {
            for item in &mut p.value {
                remap_property(item, old_to_new);
            }
        }
        Property::SetProperty(p) => {
            for item in &mut p.value.value {
                remap_property(item, old_to_new);
            }
            for item in &mut p.removed_items.value {
                remap_property(item, old_to_new);
            }
        }
        Property::MapProperty(p) => {
            for value in p.value.values_mut() {
                remap_property(value, old_to_new);
            }
        }
        Property::StructProperty(p) => {
            for item in &mut p.value {
                remap_property(item, old_to_new);
            }
        }
        _ => {}
    }
}

/// Clone an export along with all of the exports it owns, transitively or directly.
/// Rewrite references (e.g. outer indices, X_before_Y_dependencies, object properties).
pub(crate) fn deep_clone_export<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    idx: PackageIndex,
) -> PackageIndex {
    let old_indices = collect_owned_exports(umap, idx);

    let mut old_to_new = HashMap::new();
    for &old_idx in &old_indices {
        let new_idx = clone_export(umap, old_idx).expect("failed to clone export");
        old_to_new.insert(old_idx, new_idx);
    }

    for &old_idx in &old_indices {
        let new_idx = old_to_new[&old_idx];
        let export = umap.get_export_mut(new_idx).unwrap();
        let base = export.get_base_export_mut();
        remap_package_index(&mut base.class_index, &old_to_new);
        remap_package_index(&mut base.super_index, &old_to_new);
        remap_package_index(&mut base.template_index, &old_to_new);
        remap_package_index(&mut base.outer_index, &old_to_new);
        for dep in &mut base.serialization_before_serialization_dependencies {
            remap_package_index(dep, &old_to_new);
        }
        for dep in &mut base.create_before_serialization_dependencies {
            remap_package_index(dep, &old_to_new);
        }
        for dep in &mut base.serialization_before_create_dependencies {
            remap_package_index(dep, &old_to_new);
        }
        for dep in &mut base.create_before_create_dependencies {
            remap_package_index(dep, &old_to_new);
        }
        if let Some(normal) = export.get_normal_export_mut() {
            for prop in &mut normal.properties {
                remap_property(prop, &old_to_new);
            }
        }
    }

    old_to_new[&idx]
}

fn remove_actors_from_level<C: Read + Seek>(
    asset: &mut unreal_asset::Asset<C>,
    to_remove: &[PackageIndex],
) {
    let to_remove: HashSet<&PackageIndex> = to_remove.iter().collect();
    let level_idx = find_export(asset, &[with_name("PersistentLevel")]).unwrap();
    let export = asset.get_export_mut(level_idx).unwrap();
    let unreal_asset::Export::LevelExport(level_export) = export else {
        panic!("PersistentLevel was not a LevelExport");
    };
    let mut removed_idxs = vec![];
    // Preserve order: the engine requires WorldSettings to stay at Actors[0]
    // (ULevel::PostLoad does WorldSettings = Cast<AWorldSettings>(Actors[0])).
    level_export.actors.retain(|idx| {
        if !to_remove.contains(idx) {
            true
        } else {
            removed_idxs.push(*idx);
            false
        }
    });
    #[cfg(debug_assertions)]
    for idx in removed_idxs {
        let name = asset
            .get_export(idx)
            .unwrap()
            .get_base_export()
            .object_name
            .get_owned_content();
        println!("Removed {}: {} from PersistentLevel", idx, name);
    }
}

/// Turns the export subtree rooted at `root` into inert placeholder exports and removes deleted
/// exports from PersistentLevel.
pub(crate) fn deep_delete_export<C: Read + Seek>(
    umap: &mut unreal_asset::Asset<C>,
    root: PackageIndex,
) {
    let doomed = collect_owned_exports(umap, root);
    remove_actors_from_level(umap, &doomed);

    let class_idx = find_import(umap, "Class", "SceneComponent").unwrap();
    let cdo_idx = match find_import(umap, "SceneComponent", "Default__SceneComponent") {
        Some(idx) => idx,
        None => {
            // Clone an existing /Script/Engine CDO import as a pattern.
            let pattern = find_import(umap, "PlayerStart", "Default__PlayerStart").unwrap();
            let mut import = umap.get_import(pattern).unwrap().clone();
            import.class_name = umap.add_fname("SceneComponent");
            import.object_name = umap.add_fname("Default__SceneComponent");
            umap.add_import(import)
        }
    };

    let mut _tombstone_count = 0;
    for &idx in &doomed {
        let export = umap.get_export(idx).unwrap();
        let old_name = export.get_base_export().object_name.get_owned_content();
        let old_number = export.get_base_export().object_name.get_number();
        let new_name = format!("pseudochef_tombstone_{}", old_name);
        let name = umap.add_fname_with_number(&new_name, old_number);
        let export = umap.get_export_mut(idx).unwrap();
        let normal = export
            .get_normal_export_mut()
            .expect("tombstone target must be a normal export");
        normal.properties.clear();
        // A SceneComponent's body beyond tagged properties: UObject's "serialize
        // guid" flag and UActorComponent's UCSModifiedProperties array, both zero.
        normal.extras = vec![0; 8];
        let base = export.get_base_export_mut();
        base.object_name = name;
        base.class_index = class_idx;
        base.super_index = PackageIndex::new(0);
        base.template_index = cdo_idx;
        base.serialization_before_serialization_dependencies.clear();
        base.create_before_serialization_dependencies.clear();
        base.serialization_before_create_dependencies = vec![class_idx, cdo_idx];
        // create_before_create_dependencies (the outer chain) stays valid as-is.
        _tombstone_count += 1;
    }

    // Scrub surviving references to the doomed exports: drop them from
    // dependency lists and null out object/delegate properties.
    let doomed_set: HashSet<PackageIndex> = doomed.iter().copied().collect();
    let to_null: HashMap<PackageIndex, PackageIndex> = doomed
        .iter()
        .map(|&idx| (idx, PackageIndex::new(0)))
        .collect();
    for i in 0..umap.asset_data.exports.len() {
        let idx = PackageIndex::new((i + 1) as i32);
        if doomed_set.contains(&idx) {
            continue;
        }
        let export = &mut umap.asset_data.exports[i];
        let base = export.get_base_export_mut();
        base.serialization_before_serialization_dependencies
            .retain(|d| !doomed_set.contains(d));
        base.create_before_serialization_dependencies
            .retain(|d| !doomed_set.contains(d));
        base.serialization_before_create_dependencies
            .retain(|d| !doomed_set.contains(d));
        base.create_before_create_dependencies
            .retain(|d| !doomed_set.contains(d));
        if let Some(normal) = export.get_normal_export_mut() {
            for prop in &mut normal.properties {
                remap_property(prop, &to_null);
            }
        }
    }
    #[cfg(debug_assertions)]
    println!("- (deleted {} more dependent exports)", _tombstone_count - 1);
}

#[cfg(test)]
mod deep_clone_export_tests {
    use super::*;
    use crate::{MISE_UEXP, MISE_UMAP, find_export, with_name};
    use std::collections::HashSet;
    use unreal_asset::properties::Property;

    fn load_umap() -> unreal_asset::Asset<std::io::Cursor<&'static [u8]>> {
        unreal_asset::Asset::new(
            std::io::Cursor::new(MISE_UMAP),
            Some(std::io::Cursor::new(MISE_UEXP)),
            unreal_asset::engine_version::EngineVersion::VER_UE5_1,
            None,
        )
        .expect("failed to parse umap")
    }

    // Collects every PackageIndex referenced by `props`, in the same traversal
    // order used by `remap_property`, so a before/after property tree can be
    // compared reference-by-reference.
    fn collect_property_refs(props: &[Property]) -> Vec<PackageIndex> {
        let mut refs = Vec::new();
        fn visit(prop: &Property, refs: &mut Vec<PackageIndex>) {
            match prop {
                Property::ObjectProperty(p) => refs.push(p.value),
                Property::DelegateProperty(p) => refs.push(p.value.object),
                Property::MulticastDelegateProperty(p) => {
                    for delegate in &p.value {
                        refs.push(delegate.object);
                    }
                }
                Property::MulticastSparseDelegateProperty(p) => {
                    for delegate in &p.value {
                        refs.push(delegate.object);
                    }
                }
                Property::MulticastInlineDelegateProperty(p) => {
                    for delegate in &p.value {
                        refs.push(delegate.object);
                    }
                }
                Property::ArrayProperty(p) => {
                    for item in &p.value {
                        visit(item, refs);
                    }
                }
                Property::SetProperty(p) => {
                    for item in &p.value.value {
                        visit(item, refs);
                    }
                    for item in &p.removed_items.value {
                        visit(item, refs);
                    }
                }
                Property::MapProperty(p) => {
                    for value in p.value.values() {
                        visit(value, refs);
                    }
                }
                Property::StructProperty(p) => {
                    for item in &p.value {
                        visit(item, refs);
                    }
                }
                _ => {}
            }
        }
        for prop in props {
            visit(prop, &mut refs);
        }
        refs
    }

    // Applies the same remap rule `deep_clone_export` uses: indices inside the
    // cloned subtree follow `old_to_new`, everything else (imports, or exports
    // outside the subtree, like the owning level) stays as-is.
    fn expected_remap(
        old: PackageIndex,
        old_to_new: &HashMap<PackageIndex, PackageIndex>,
    ) -> PackageIndex {
        old_to_new.get(&old).copied().unwrap_or(old)
    }

    // Deep-clones `object_name` in `umap` and checks that:
    //  - exactly one new export is added per export in the original's owned
    //    subtree (the export itself plus every export outered to it),
    //  - every clone's base fields (outer/class/super/template index and the
    //    four dependency lists) and object properties point at the *other*
    //    clones in the new subtree wherever the original pointed within its own
    //    subtree, and are left unchanged (still pointing at imports / exports
    //    outside the subtree) everywhere else,
    //  - the clones get fresh, distinct names,
    //  - the original subtree is left completely untouched.
    fn assert_deep_clone_is_self_contained(object_name: &str, expected_new_export_count: usize) {
        let mut umap = load_umap();
        let root = find_export(&umap, &vec![with_name(object_name)]).unwrap();

        let old_subtree = collect_owned_exports(&umap, root);
        assert_eq!(
            old_subtree.len(),
            expected_new_export_count,
            "unexpected owned-subtree size for {}",
            object_name
        );

        // Snapshot the original subtree so we can confirm it's untouched by the clone.
        let originals: Vec<_> = old_subtree
            .iter()
            .map(|&idx| umap.get_export(idx).unwrap().clone())
            .collect();

        let export_count_before = umap.asset_data.exports.len();
        let new_root = deep_clone_export(&mut umap, root);
        let export_count_after = umap.asset_data.exports.len();

        assert_eq!(
            export_count_after,
            export_count_before + expected_new_export_count,
            "deep_clone_export({}) should add exactly {} new exports",
            object_name,
            expected_new_export_count
        );

        let new_subtree = collect_owned_exports(&umap, new_root);
        assert_eq!(
            new_subtree.len(),
            old_subtree.len(),
            "cloned subtree shape doesn't match original"
        );

        let old_set: HashSet<PackageIndex> = old_subtree.iter().copied().collect();
        let new_set: HashSet<PackageIndex> = new_subtree.iter().copied().collect();
        assert!(
            old_set.is_disjoint(&new_set),
            "cloned subtree must not overlap the original"
        );
        assert!(
            new_subtree
                .iter()
                .all(|idx| idx.index > export_count_before as i32),
            "every cloned export should be newly appended"
        );

        let old_to_new: HashMap<PackageIndex, PackageIndex> = old_subtree
            .iter()
            .copied()
            .zip(new_subtree.iter().copied())
            .collect();

        for (i, (&old_idx, &new_idx)) in old_subtree.iter().zip(new_subtree.iter()).enumerate() {
            let old_export = &originals[i];
            let old_base = old_export.get_base_export();
            let new_export = umap.get_export(new_idx).unwrap();
            let new_base = new_export.get_base_export();

            // Fresh, distinct name.
            assert_ne!(
                new_base.object_name.get_owned_content(),
                old_base.object_name.get_owned_content()
            );
            assert!(
                new_base
                    .object_name
                    .get_owned_content()
                    .contains(&old_base.object_name.get_owned_content())
            );

            // outer/class/super/template indices follow the same remap rule.
            assert_eq!(
                new_base.class_index,
                expected_remap(old_base.class_index, &old_to_new)
            );
            assert_eq!(
                new_base.super_index,
                expected_remap(old_base.super_index, &old_to_new)
            );
            assert_eq!(
                new_base.template_index,
                expected_remap(old_base.template_index, &old_to_new)
            );
            assert_eq!(
                new_base.outer_index,
                expected_remap(old_base.outer_index, &old_to_new)
            );

            // The four X_before_Y_dependencies fields, remapped entry-by-entry.
            let old_deps = [
                &old_base.serialization_before_serialization_dependencies,
                &old_base.create_before_serialization_dependencies,
                &old_base.serialization_before_create_dependencies,
                &old_base.create_before_create_dependencies,
            ];
            let new_deps = [
                &new_base.serialization_before_serialization_dependencies,
                &new_base.create_before_serialization_dependencies,
                &new_base.serialization_before_create_dependencies,
                &new_base.create_before_create_dependencies,
            ];
            for (old_dep_list, new_dep_list) in old_deps.iter().zip(new_deps.iter()) {
                let expected: Vec<PackageIndex> = old_dep_list
                    .iter()
                    .map(|&d| expected_remap(d, &old_to_new))
                    .collect();
                assert_eq!(**new_dep_list, expected, "at export {:?}", old_idx);
            }

            // Object properties, remapped reference-by-reference.
            let old_refs = old_export
                .get_normal_export()
                .map(|n| collect_property_refs(&n.properties))
                .unwrap_or_default();
            let new_refs = new_export
                .get_normal_export()
                .map(|n| collect_property_refs(&n.properties))
                .unwrap_or_default();
            let expected_refs: Vec<PackageIndex> = old_refs
                .iter()
                .map(|&r| expected_remap(r, &old_to_new))
                .collect();
            assert_eq!(
                new_refs, expected_refs,
                "property refs at export {:?}",
                old_idx
            );
        }

        // The original subtree must be completely unmodified.
        for (i, &old_idx) in old_subtree.iter().enumerate() {
            assert_eq!(umap.get_export(old_idx).unwrap(), &originals[i]);
        }
    }

    // BP_Hazard_C is a small actor: a DefaultSceneRoot (SceneComponent) plus a
    // StaticMeshComponent attached to it, both outered directly to the actor.
    // Deep-cloning it must therefore add exactly 3 exports: the actor itself and
    // its two owned components.
    #[test]
    fn deep_clone_bp_hazard_c() {
        assert_deep_clone_is_self_contained("BP_Hazard_C", 3);
    }

    // BP_JumpBubble_C owns a DefaultSceneRoot, a SphereComponent and a
    // StaticMeshComponent, so cloning it adds 4 exports.
    #[test]
    fn deep_clone_bp_jump_bubble_c() {
        assert_deep_clone_is_self_contained("BP_JumpBubble_C", 4);
    }

    // BP_SavePoint_C owns a DefaultSceneRoot, a SphereComponent, a
    // StaticMeshComponent, a BoxComponent, and a nested BP_HpHitable_C child
    // actor, so cloning it adds 6 exports.
    #[test]
    fn deep_clone_bp_save_point_c() {
        assert_deep_clone_is_self_contained("BP_SavePoint_C", 6);
    }

    // Cloning twice must not collide: each call produces its own independent,
    // uniquely-named subtree.
    #[test]
    fn deep_clone_twice_is_independent() {
        let mut umap = load_umap();
        let root = find_export(&umap, &vec![with_name("BP_Hazard_C")]).unwrap();

        let clone_a = deep_clone_export(&mut umap, root);
        let clone_b = deep_clone_export(&mut umap, root);
        assert_ne!(clone_a, clone_b);

        // Content (base string) is expected to match ("pseudochef_BP_Hazard_C" for
        // both); uniqueness comes from the FName instance number instead.
        let name_a = umap
            .get_export(clone_a)
            .unwrap()
            .get_base_export()
            .object_name
            .clone();
        let name_b = umap
            .get_export(clone_b)
            .unwrap()
            .get_base_export()
            .object_name
            .clone();
        assert_eq!(name_a.get_owned_content(), name_b.get_owned_content());
        assert_ne!(name_a.get_number(), name_b.get_number());

        let subtree_a: HashSet<_> = collect_owned_exports(&umap, clone_a).into_iter().collect();
        let subtree_b: HashSet<_> = collect_owned_exports(&umap, clone_b).into_iter().collect();
        assert!(subtree_a.is_disjoint(&subtree_b));
    }
}

#[cfg(test)]
mod deep_delete_export_tests {
    use super::*;
    use crate::{MISE_UEXP, MISE_UMAP};

    fn load_umap() -> unreal_asset::Asset<std::io::Cursor<&'static [u8]>> {
        unreal_asset::Asset::new(
            std::io::Cursor::new(MISE_UMAP),
            Some(std::io::Cursor::new(MISE_UEXP)),
            unreal_asset::engine_version::EngineVersion::VER_UE5_1,
            None,
        )
        .expect("failed to parse umap")
    }

    // Deleting an actor must leave a package that unreal_asset can round-trip, with the same
    // export count (indices stable), no export left whose class or name identifies the removed
    // actor, and no surviving reference (level actor list, dependency lists, properties) to the
    // doomed subtree.
    #[test]
    fn deep_delete_bp_jump_bubble_c_round_trips() {
        let mut umap = load_umap();
        let root = find_export(&umap, &vec![with_name("BP_JumpBubble_C")]).unwrap();
        let doomed = collect_owned_exports(&umap, root);
        assert_eq!(doomed.len(), 4);
        let export_count = umap.asset_data.exports.len();

        deep_delete_export(&mut umap, root);

        let mut umap_bytes = std::io::Cursor::new(vec![]);
        let mut uexp_bytes = std::io::Cursor::new(vec![]);
        umap.write_data(&mut umap_bytes, Some(&mut uexp_bytes))
            .expect("failed to serialize deleted umap");

        let reloaded = unreal_asset::Asset::new(
            std::io::Cursor::new(umap_bytes.into_inner()),
            Some(std::io::Cursor::new(uexp_bytes.into_inner())),
            unreal_asset::engine_version::EngineVersion::VER_UE5_1,
            None,
        )
        .expect("failed to re-parse deleted umap");

        assert_eq!(reloaded.asset_data.exports.len(), export_count);
        assert!(find_export(&reloaded, &vec![with_name("BP_JumpBubble_C")]).is_none());

        let scene_component_class = find_import(&mut umap, "Class", "SceneComponent").unwrap();
        for &idx in &doomed {
            let base = reloaded.get_export(idx).unwrap().get_base_export();
            assert_eq!(base.class_index, scene_component_class);
            assert!(
                base.object_name
                    .get_content(|content| content.starts_with("pseudochef_tombstone"))
            );
            let normal = reloaded
                .get_export(idx)
                .unwrap()
                .get_normal_export()
                .unwrap();
            assert!(normal.properties.is_empty());
        }

        // No surviving export may reference the doomed subtree.
        let doomed_set: HashSet<PackageIndex> = doomed.iter().copied().collect();
        for (i, export) in reloaded.asset_data.exports.iter().enumerate() {
            let idx = PackageIndex::new((i + 1) as i32);
            if doomed_set.contains(&idx) {
                continue;
            }
            let base = export.get_base_export();
            for deps in [
                &base.serialization_before_serialization_dependencies,
                &base.create_before_serialization_dependencies,
                &base.serialization_before_create_dependencies,
                &base.create_before_create_dependencies,
            ] {
                assert!(deps.iter().all(|d| !doomed_set.contains(d)));
            }
            if let unreal_asset::Export::LevelExport(level) = export {
                assert!(level.actors.iter().all(|a| !doomed_set.contains(a)));
            }
        }
    }
}
