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

        add("MatTex/Textures/Environment/Cave_Brick.ppm", 64, 64);
        add("MatTex/Textures/Environment/Red_floor.ppm", 64, 64);
        add("MatTex/Textures/Environment/Theater_Floor.ppm", 64, 64);
        add("MatTex/Textures/Environment/artDecoPattern3.ppm", 64, 62);
        add(
            "MatTex/Textures/Environment/building_door_window.ppm",
            64,
            64,
        );
        add("MatTex/Textures/Environment/building_floor_01.ppm", 64, 64);
        add("MatTex/Textures/Environment/building_wall_02.ppm", 64, 64);
        add("MatTex/Textures/Environment/building_wall_03.ppm", 64, 64);
        add("MatTex/Textures/Environment/building_wall_04.ppm", 64, 64);
        add("MatTex/Textures/Environment/building_wall_05.ppm", 32, 32);
        add("MatTex/Textures/Environment/building_wall_06.ppm", 32, 32);
        add("MatTex/Textures/Environment/building_wall_07.ppm", 32, 32);
        add("MatTex/Textures/Environment/building_wall_08.ppm", 32, 32);
        add("MatTex/Textures/Environment/roofShingles.ppm", 64, 64);
        add("MatTex/Textures/Environment/weirdWall.ppm", 64, 64);
        add("MatTex/Textures/Environment/wooden_boards.ppm", 64, 32);
        add("MatTex/Textures/T_FluidWater.ppm", 64, 64);
        add("MatTex/Textures/T_WaterSurface.ppm", 64, 64);
        add("MatTex/Textures/big_brick.ppm", 64, 64);
        add("MatTex/Textures/cave_brick.ppm", 32, 32);
        add("MatTex/Textures/cavetexture.ppm", 64, 64);
        add("MatTex/Textures/checkerTile.ppm", 32, 32);
        add("MatTex/Textures/fence.ppm", 32, 32);
        add("MatTex/Textures/marble_texture.ppm", 64, 64);
        add("MatTex/Textures/moon0005.ppm", 32, 32);
        add("MatTex/Textures/moon0008.ppm", 32, 32);
        add("MatTex/Textures/moon0009.ppm", 32, 32);
        add("MatTex/Textures/moon0010.ppm", 32, 32);
        add("MatTex/Textures/moon0011.ppm", 32, 32);
        add("MatTex/Textures/moon0012.ppm", 32, 32);
        add("MatTex/Textures/moon0013.ppm", 32, 32);
        add("MatTex/Textures/moon0014.ppm", 32, 32);
        add("MatTex/Textures/moon0015.ppm", 32, 32);
        add("MatTex/Textures/moon0016.ppm", 32, 32);
        add("MatTex/Textures/moon0017.ppm", 32, 32);
        add("MatTex/Textures/moon0018.ppm", 32, 32);
        add("MatTex/Textures/moon0023.ppm", 32, 32);
        add("MatTex/Textures/moon0024.ppm", 32, 32);
        add("MatTex/Textures/moon0025.ppm", 32, 32);
        add("MatTex/Textures/moon0026.ppm", 32, 32);
        add("MatTex/Textures/moontexture0000.ppm", 32, 32);
        add("MatTex/Textures/moontexture0003.ppm", 32, 32);
        add("MatTex/Textures/moontexture0005.ppm", 32, 32);
        add("MatTex/Textures/moontexture0006.ppm", 32, 32);
        add("MatTex/Textures/moontexture0007.ppm", 32, 32);
        add("MatTex/Textures/moontexture0008.ppm", 32, 32);
        add("MatTex/Textures/moontexture0010.ppm", 32, 32);
        add("MatTex/Textures/moontexture0011.ppm", 32, 32);

        Self { m: Some(m) }
    }
}
