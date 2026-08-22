#[derive(Default, Debug)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub texture_format: String,
    pub data: Vec<u8>,
}

impl ImageData {
    pub fn write_png<W: std::io::Write>(&self, dst: &mut W) -> std::io::Result<()> {
        let mut data = vec![];
        for bgra in self.data.chunks(4) {
            let b = bgra.get(0).unwrap_or(&0);
            let g = bgra.get(1).unwrap_or(&0);
            let r = bgra.get(2).unwrap_or(&0);
            let a = bgra.get(3).unwrap_or(&0);
            data.push(*r);
            data.push(*g);
            data.push(*b);
            data.push(*a);
        }
        let mut encoder = png::Encoder::new(dst, self.width, self.height);

        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        // Copied from sample code in png crate docs
        encoder.set_source_gamma(png::ScaledFloat::new(1.0 / 2.2));
        let source_chromaticities = png::SourceChromaticities::new(
            (0.31270, 0.32900),
            (0.64000, 0.33000),
            (0.30000, 0.60000),
            (0.15000, 0.06000),
        );
        encoder.set_source_chromaticities(source_chromaticities);

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&data)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct Error {
    pub msg: String,
}

impl Error {
    fn new(msg: &str) -> Error {
        Error {
            msg: msg.to_string(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for Error {}

macro_rules! err {
    ($($arg:tt)*) => {
        Err(Box::new(Error { msg: format!($($arg)*) }))
    }
}

fn parse_u32(bytes: &[u8], offset: usize) -> Result<u32, Box<dyn std::error::Error>> {
    if bytes.len() < offset + 4 {
        return err!(
            "failed to parse u32 at offset {} in buffer of size {}",
            offset,
            bytes.len()
        );
    }
    let n = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    Ok(n)
}

pub fn decode(bytes: &[u8]) -> Result<ImageData, Box<dyn std::error::Error>> {
    const OFFSET_DIM: usize = 0x2c;
    const OFFSET_TEX_FMT: usize = 0x3c;
    const OFFSET_DATA: usize = 0x64;
    let width = parse_u32(bytes, OFFSET_DIM)?;
    let height = parse_u32(bytes, OFFSET_DIM + 4)?;

    let data_size = (width * height * 4) as usize;
    let data_end_idx = OFFSET_DATA + data_size;

    let null_byte_idx = bytes[OFFSET_TEX_FMT..]
        .iter()
        .position(|b| *b == 0)
        .ok_or(Error::new(
            "texture format not null-terminated (input corrupted)",
        ))?
        + OFFSET_TEX_FMT;
    let texture_format = &bytes[OFFSET_TEX_FMT..null_byte_idx];
    let texture_format = String::from_utf8(Vec::from(texture_format))?;
    if texture_format != "PF_B8G8R8A8" {
        return err!("unsupported format: {}", texture_format);
    }

    let data = Vec::from(&bytes[OFFSET_DATA..data_end_idx]);

    Ok(ImageData {
        width,
        height,
        texture_format,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_on_empty_input() {
        println!("{}", decode(&[]).unwrap_err());
    }
}
