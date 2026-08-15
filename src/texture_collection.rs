use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct Texture {
    pub width: u16,
    pub height: u16,
}

impl Texture {
    const fn not_found() -> Self {
        Self {
            width: 16,
            height: 16,
        }
    }
}

const NOT_FOUND: Texture = Texture::not_found();

pub struct TextureCollection {
    m: Option<HashMap<&'static str, Texture>>,
}

impl TextureCollection {
    pub fn get(&self, name: &str) -> &Texture {
        let Some(m) = &self.m else {
            return &NOT_FOUND;
        };
        let Some(t) = m.get(name) else {
            return &NOT_FOUND;
        };
        &t
    }

    // Used in tests
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { m: None }
    }

    pub fn bundled() -> Self {
        let mut m: HashMap<&'static str, Texture> = HashMap::new();
        m.insert(
            "MatTex/Textures/Environment/Cave_Brick",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/Red_floor",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/Theater_Floor",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/artDecoPattern3",
            Texture {
                width: 64,
                height: 62,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/building_door_window",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/building_floor_01",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/building_wall_02",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/building_wall_03",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/building_wall_04",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/building_wall_05",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/building_wall_06",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/building_wall_07",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/building_wall_08",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/roofShingles",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/weirdWall",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/Environment/wooden_boards",
            Texture {
                width: 64,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/T_FluidWater",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/T_WaterSurface",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/big_brick",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/cave_brick",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/cavetexture",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/checkerTile",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/fence",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/marble_texture",
            Texture {
                width: 64,
                height: 64,
            },
        );
        m.insert(
            "MatTex/Textures/moon0005",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0008",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0009",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0010",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0011",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0012",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0013",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0014",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0015",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0016",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0017",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0018",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0023",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0024",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0025",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moon0026",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moontexture0000",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moontexture0003",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moontexture0005",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moontexture0006",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moontexture0007",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moontexture0008",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moontexture0010",
            Texture {
                width: 32,
                height: 32,
            },
        );
        m.insert(
            "MatTex/Textures/moontexture0011",
            Texture {
                width: 32,
                height: 32,
            },
        );
        Self { m: Some(m) }
    }
}
