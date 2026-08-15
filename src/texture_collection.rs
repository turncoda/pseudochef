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
            "MatTex/Textures/big_brick",
            Texture {
                width: 64,
                height: 64,
            },
        );
        Self { m: Some(m) }
    }
}
