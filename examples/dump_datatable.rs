// Temporary debugging tool: dump the rows of a DataTable uasset.
// Usage: cargo run --example dump_datatable <path-without-extension>
use std::io::Cursor;

fn main() {
    let base = std::env::args().nth(1).expect("usage: dump_datatable <path-without-extension>");
    let uasset = std::fs::read(format!("{base}.uasset")).expect("failed to read uasset");
    let uexp = std::fs::read(format!("{base}.uexp")).expect("failed to read uexp");

    let asset = unreal_asset::Asset::new(
        Cursor::new(uasset),
        Some(Cursor::new(uexp)),
        unreal_asset::engine_version::EngineVersion::VER_UE5_1,
        None,
    )
    .expect("failed to parse asset");

    println!("=== IMPORTS ===");
    for (i, import) in asset.imports.iter().enumerate() {
        println!(
            "[-{}] class={} name={} outer={}",
            i + 1,
            import.class_name.get_owned_content(),
            import.object_name.get_owned_content(),
            import.outer_index.index,
        );
    }

    let resolve = |idx: unreal_asset::types::PackageIndex| -> String {
        if idx.index == 0 {
            return "<null>".to_string();
        }
        asset
            .get_import(idx)
            .map(|i| i.object_name.get_owned_content())
            .unwrap_or("?".to_string())
    };

    println!("\n=== ROWS ===");
    for export in &asset.asset_data.exports {
        let unreal_asset::exports::Export::DataTableExport(dt) = export else {
            continue;
        };
        for row in &dt.table.data {
            println!("row={}", row.name.get_owned_content());
            for prop in &row.value {
                use unreal_asset::properties::Property;
                let name = match prop {
                    Property::IntProperty(p) => {
                        println!("  {}={}", p.name.get_owned_content(), p.value);
                        continue;
                    }
                    Property::TextProperty(p) => {
                        println!(
                            "  {}={:?}",
                            p.name.get_owned_content(),
                            p.culture_invariant_string
                        );
                        continue;
                    }
                    Property::ObjectProperty(p) => {
                        println!("  {}={}", p.name.get_owned_content(), resolve(p.value));
                        continue;
                    }
                    Property::ArrayProperty(p) => {
                        println!("  {}=[", p.name.get_owned_content());
                        for elem in &p.value {
                            if let Property::TextProperty(t) = elem {
                                println!("    {:?}", t.culture_invariant_string);
                            } else {
                                println!("    {:?}", elem);
                            }
                        }
                        println!("  ]");
                        continue;
                    }
                    other => format!("{:?}", other),
                };
                println!("  {}", name);
            }
        }
    }
}
