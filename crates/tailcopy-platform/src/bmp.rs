//! A BMP file (for `CF_BITMAP`) from PNG bytes. It always uses the plain 40-byte
//! `BITMAPINFOHEADER` with 32-bit BI_RGB pixels: clipboard-win copies exactly that
//! header before calling `CreateDIBitmap`, so a larger header (the BITMAPV4HEADER the
//! `image` encoder writes for RGBA) would make GDI read past it.

const FILE_HEADER: u32 = 14;
const INFO_HEADER: u32 = 40;

pub(crate) fn png_to_bmp(png: &[u8]) -> Result<Vec<u8>, image::ImageError> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)?.to_rgba8();
    let (width, height) = img.dimensions();
    let row = width as usize * 4;
    let pixels = row * height as usize;
    let offset = FILE_HEADER + INFO_HEADER;
    let mut out = Vec::with_capacity(offset as usize + pixels);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(offset + pixels as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(&INFO_HEADER.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes()); // bottom-up rows
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(pixels as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes()); // 72 dpi
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&[0; 8]); // palette counts
    for y in (0..height).rev() {
        for x in 0..width {
            let [r, g, b, a] = img.get_pixel(x, y).0;
            out.extend_from_slice(&[b, g, r, a]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn writes_a_40_byte_header() {
        let mut png = Vec::new();
        let img = image::RgbaImage::from_fn(2, 3, |x, y| image::Rgba([x as u8, y as u8, 7, 255]));
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let bmp = super::png_to_bmp(&png).unwrap();
        let u32_at = |i: usize| u32::from_le_bytes(bmp[i..i + 4].try_into().unwrap());
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(u32_at(10), 54, "pixel data offset");
        assert_eq!(u32_at(14), 40, "biSize");
        assert_eq!(u32_at(30), 0, "BI_RGB");
        assert_eq!(bmp.len(), 54 + 2 * 3 * 4);
        // First stored row is the bottom one (y = 2): pixel (0, 2) as BGRA.
        assert_eq!(&bmp[54..58], &[7, 2, 0, 255]);
    }
}
