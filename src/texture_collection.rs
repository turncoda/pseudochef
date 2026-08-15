use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Texture {
    pub width: u16,
    pub height: u16,
    pub material_path: Option<String>,
}

impl Texture {
    const fn not_found() -> Self {
        Self {
            width: 16,
            height: 16,
            material_path: None,
        }
    }
}

const NOT_FOUND: Texture = Texture::not_found();

#[derive(Debug)]
pub struct TextureCollection {
    m: Option<HashMap<&'static str, Texture>>,
}

impl TextureCollection {
    pub fn iter_mut(&mut self) -> std::collections::hash_map::IterMut<'_, &str, Texture> {
        let Some(m) = &mut self.m else {
            return std::collections::hash_map::IterMut::default();
        };
        m.iter_mut()
    }
    pub fn get(&self, name: &str) -> Texture {
        let Some(m) = &self.m else {
            return NOT_FOUND;
        };
        let Some(t) = m.get(name) else {
            return NOT_FOUND;
        };
        t.clone()
    }

    // Used in tests
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { m: None }
    }

    pub fn bundled() -> Self {
        let mut m: HashMap<&'static str, Texture> = HashMap::new();
        let mut add = |name, width, height| {
            m.insert(
                name,
                Texture {
                    width,
                    height,
                    material_path: None,
                },
            );
        };

        add("MatTex/Textures/Environment/Cave_Brick", 64, 64);
        add("MatTex/Textures/Environment/Red_floor", 64, 64);
        add("MatTex/Textures/Environment/Theater_Floor", 64, 64);
        add("MatTex/Textures/Environment/artDecoPattern3", 64, 62);
        add("MatTex/Textures/Environment/building_door_window", 64, 64);
        add("MatTex/Textures/Environment/building_floor_01", 64, 64);
        add("MatTex/Textures/Environment/building_wall_02", 64, 64);
        add("MatTex/Textures/Environment/building_wall_03", 64, 64);
        add("MatTex/Textures/Environment/building_wall_04", 64, 64);
        add("MatTex/Textures/Environment/building_wall_05", 32, 32);
        add("MatTex/Textures/Environment/building_wall_06", 32, 32);
        add("MatTex/Textures/Environment/building_wall_07", 32, 32);
        add("MatTex/Textures/Environment/building_wall_08", 32, 32);
        add("MatTex/Textures/Environment/roofShingles", 64, 64);
        add("MatTex/Textures/Environment/weirdWall", 64, 64);
        add("MatTex/Textures/Environment/wooden_boards", 64, 32);
        add("MatTex/Textures/T_FluidWater", 64, 64);
        add("MatTex/Textures/T_WaterSurface", 64, 64);
        add("MatTex/Textures/big_brick", 64, 64);
        add("MatTex/Textures/cave_brick", 32, 32);
        add("MatTex/Textures/cavetexture", 64, 64);
        add("MatTex/Textures/checkerTile", 32, 32);
        add("MatTex/Textures/fence", 32, 32);
        add("MatTex/Textures/marble_texture", 64, 64);
        add("MatTex/Textures/moon0005", 32, 32);
        add("MatTex/Textures/moon0008", 32, 32);
        add("MatTex/Textures/moon0009", 32, 32);
        add("MatTex/Textures/moon0010", 32, 32);
        add("MatTex/Textures/moon0011", 32, 32);
        add("MatTex/Textures/moon0012", 32, 32);
        add("MatTex/Textures/moon0013", 32, 32);
        add("MatTex/Textures/moon0014", 32, 32);
        add("MatTex/Textures/moon0015", 32, 32);
        add("MatTex/Textures/moon0016", 32, 32);
        add("MatTex/Textures/moon0017", 32, 32);
        add("MatTex/Textures/moon0018", 32, 32);
        add("MatTex/Textures/moon0023", 32, 32);
        add("MatTex/Textures/moon0024", 32, 32);
        add("MatTex/Textures/moon0025", 32, 32);
        add("MatTex/Textures/moon0026", 32, 32);
        add("MatTex/Textures/moontexture0000", 32, 32);
        add("MatTex/Textures/moontexture0003", 32, 32);
        add("MatTex/Textures/moontexture0005", 32, 32);
        add("MatTex/Textures/moontexture0006", 32, 32);
        add("MatTex/Textures/moontexture0007", 32, 32);
        add("MatTex/Textures/moontexture0008", 32, 32);
        add("MatTex/Textures/moontexture0010", 32, 32);
        add("MatTex/Textures/moontexture0011", 32, 32);

        Self { m: Some(m) }
    }
}
