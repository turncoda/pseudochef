// Temporary debugging tool: dump the export/import tables of a umap.
use std::io::Cursor;
use unreal_asset::exports::ExportBaseTrait;
use unreal_asset::types::PackageIndex;

const UMAP: &[u8] = include_bytes!("../src/base/ModPack_Base.umap");
const UEXP: &[u8] = include_bytes!("../src/base/ModPack_Base.uexp");

fn main() {
    let umap = unreal_asset::Asset::new(
        Cursor::new(UMAP),
        Some(Cursor::new(UEXP)),
        unreal_asset::engine_version::EngineVersion::VER_UE5_1,
        None,
    )
    .expect("failed to parse umap");

    println!("=== IMPORTS ===");
    for (i, import) in umap.imports.iter().enumerate() {
        println!(
            "[-{}] class={} name={} outer={}",
            i + 1,
            import.class_name.get_owned_content(),
            import.object_name.get_owned_content(),
            import.outer_index.index,
        );
    }

    let resolve = |idx: PackageIndex| -> String {
        if idx.index == 0 {
            return "<null>".to_string();
        }
        if idx.index < 0 {
            return format!(
                "import:{}",
                umap.get_import(idx)
                    .map(|i| i.object_name.get_owned_content())
                    .unwrap_or("?".to_string())
            );
        }
        umap.get_export(idx)
            .map(|e| e.get_base_export().object_name.get_owned_content())
            .unwrap_or("?".to_string())
    };

    println!("\n=== EXPORTS ===");
    for (i, export) in umap.asset_data.exports.iter().enumerate() {
        let b = export.get_base_export();
        println!(
            "[{}] name={} class={} outer={}({}) flags={:?}",
            i + 1,
            b.object_name.get_owned_content(),
            resolve(b.class_index),
            b.outer_index.index,
            resolve(b.outer_index),
            b.object_flags,
        );
        let deps = [
            ("ser_before_ser", &b.serialization_before_serialization_dependencies),
            ("create_before_ser", &b.create_before_serialization_dependencies),
            ("ser_before_create", &b.serialization_before_create_dependencies),
            ("create_before_create", &b.create_before_create_dependencies),
        ];
        for (name, list) in deps {
            if !list.is_empty() {
                let items: Vec<String> = list
                    .iter()
                    .map(|d| format!("{}({})", d.index, resolve(*d)))
                    .collect();
                println!("      {}: {}", name, items.join(", "));
            }
        }
    }

    println!("\n=== PROPERTIES OF SELECTED EXPORTS ===");
    use unreal_asset::exports::ExportNormalTrait;
    for &i in &[5, 1, 3, 18, 10, 21, 24, 32] {
        let export = umap.get_export(PackageIndex::new(i)).unwrap();
        let b = export.get_base_export();
        println!(
            "[{}] {} ({})",
            i,
            b.object_name.get_owned_content(),
            resolve(b.class_index)
        );
        if let Some(normal) = export.get_normal_export() {
            for prop in &normal.properties {
                println!("    {:?}", prop);
            }
        }
    }

    println!("\n=== EXTRAS ===");
    for (i, export) in umap.asset_data.exports.iter().enumerate() {
        if let Some(normal) = export.get_normal_export() {
            println!(
                "[{}] {} extras={:?}",
                i + 1,
                normal.base_export.object_name.get_owned_content(),
                normal.extras,
            );
        }
    }

    println!("\n=== LEVEL EXPORT ===");
    for export in &umap.asset_data.exports {
        if let unreal_asset::Export::LevelExport(level) = export {
            println!(
                "level: {}",
                level.normal_export.base_export.object_name.get_owned_content()
            );
            println!("actors ({}):", level.actors.len());
            for a in &level.actors {
                println!("  {}({})", a.index, resolve(*a));
            }
            println!("model: {}({})", level.model.index, resolve(level.model));
            println!("model_components: {:?}", level.model_components);
            println!(
                "level_script: {}({})",
                level.level_script.index,
                resolve(level.level_script)
            );
            println!(
                "nav_list_start: {} nav_list_end: {}",
                level.nav_list_start.index, level.nav_list_end.index
            );
        }
    }
}
