//! `pseudocooker` -- UE5.1 static mesh cooker
//!
//! Input:
//! Takes raw mesh data as input and outputs cooked UE5.1-compliant static mesh uasset.
//!
//! Overview:
//!   - BodySetup, including PhysXPC collision ([`bodysetup`], [`physxpc`])
//!   - NavCollision ([`navcollision`])
//!   - StaticMesh, including FStaticMeshRenderData ([`staticmesh`])
//!   - Package assembly: name/import/export tables + header ([`package`])
//!
//! Known limitations:
//!   - Nanite, Lumen card data, mesh distance fields, ray tracing geometry omitted
//!   - NavCollision's "NavCollision_Chaos" blob format omitted
//!   - Single LOD

pub mod bodysetup;
pub mod core;
pub mod mesh;
pub mod navcollision;
pub mod package;
pub mod physxpc;
pub mod staticmesh;

pub use mesh::{Bounds, Corner, Face, MeshInput, RenderMesh, Section, Tangent, Vec2, Vec3};

pub struct CookedAsset {
    pub uasset: Vec<u8>,
    pub uexp: Vec<u8>,
}

impl CookedAsset {
    pub fn write_to_dir(&self, dir: &std::path::Path, asset_name: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(format!("{asset_name}.uasset")), &self.uasset)?;
        std::fs::write(dir.join(format!("{asset_name}.uexp")), &self.uexp)?;
        Ok(())
    }
}

/// Expects input mesh data to be in UE-space:
/// - Distance units are centimeters
/// - Left-handed coordinate system
/// - +Z axis pointing up
/// Returns bytes that would be written to the .uasset, .uexp, and .ubulk files.
pub fn cook(mesh_input: &MeshInput, asset_name: &str) -> CookedAsset {
    let render_mesh = mesh::build_render_mesh(mesh_input);
    let (uasset, uexp) = package::cook_package(asset_name, &render_mesh);
    CookedAsset { uasset, uexp }
}
