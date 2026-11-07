//! Pictures for quick view: PNG and JPEG, decoded and shrunk to a grid of colours the
//! terminal draws with half blocks (`▀`, two pixels a cell). No graphics protocol, so
//! it works over SSH in any terminal with true colour.
//!
//! An image file is someone else's bytes, and its header says how much memory to
//! allocate. Both are bounded before anything is decoded: the file's size, and the
//! pixel count from the header — a 50000×50000 PNG of a few kilobytes would otherwise
//! ask for ten gigabytes. The decoders are pure Rust.

use std::fs;
use std::io::{self, BufReader, Cursor};
use std::path::Path;

const MAX_FILE: u64 = 64 << 20;
const MAX_PIXELS: u64 = 50_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    /// The size of the original, for the title.
    pub source: (usize, usize),
    pub rgb: Vec<[u8; 3]>,
}

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"))
}

fn too_big(w: u64, h: u64) -> io::Result<()> {
    if w.saturating_mul(h) > MAX_PIXELS {
        return Err(io::Error::other(format!(
            "{w}\u{d7}{h} is too large to preview"
        )));
    }
    Ok(())
}

/// Decode `path` and shrink it to fit `max_w` × `max_h` pixels, keeping its shape.
/// Transparency is laid over `bg`.
pub fn load(path: &Path, max_w: usize, max_h: usize, bg: [u8; 3]) -> io::Result<Picture> {
    if fs::metadata(path)?.len() > MAX_FILE {
        return Err(io::Error::other(
            "larger than 64 MiB \u{2014} not previewed",
        ));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (w, h, px) = if ext == "png" {
        png(BufReader::new(fs::File::open(path)?), bg)?
    } else {
        jpeg(path)?
    };
    Ok(shrink(w, h, &px, max_w.max(1), max_h.max(1)))
}

/// A PNG already in memory — xdevcmd's own logo, built into the binary — shrunk to fit as
/// [`load`] does.
pub fn png_bytes(bytes: &[u8], max_w: usize, max_h: usize, bg: [u8; 3]) -> io::Result<Picture> {
    let (w, h, px) = png(Cursor::new(bytes), bg)?;
    Ok(shrink(w, h, &px, max_w.max(1), max_h.max(1)))
}

fn png<R: io::BufRead + io::Seek>(r: R, bg: [u8; 3]) -> io::Result<(usize, usize, Vec<[u8; 3]>)> {
    let mut d = png::Decoder::new(r);
    d.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = d.read_info().map_err(io::Error::other)?;
    let (w, h) = (r.info().width as u64, r.info().height as u64);
    too_big(w, h)?;
    let size = r
        .output_buffer_size()
        .ok_or_else(|| io::Error::other("an image too large to decode"))?;
    let mut buf = vec![0u8; size];
    let info = r.next_frame(&mut buf).map_err(io::Error::other)?;
    let bytes = &buf[..info.buffer_size()];
    let over =
        |c: u8, a: u8, b: u8| ((c as u16 * a as u16 + b as u16 * (255 - a as u16)) / 255) as u8;
    let px: Vec<[u8; 3]> = match info.color_type {
        png::ColorType::Rgb => bytes
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| [p[0], p[1], p[2]])
            .collect(),
        png::ColorType::Rgba => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                [
                    over(p[0], p[3], bg[0]),
                    over(p[1], p[3], bg[1]),
                    over(p[2], p[3], bg[2]),
                ]
            })
            .collect(),
        png::ColorType::Grayscale => bytes.iter().map(|g| [*g, *g, *g]).collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                [
                    over(p[0], p[1], bg[0]),
                    over(p[0], p[1], bg[1]),
                    over(p[0], p[1], bg[2]),
                ]
            })
            .collect(),
        png::ColorType::Indexed => return Err(io::Error::other("unexpanded palette")),
    };
    Ok((w as usize, h as usize, px))
}

fn jpeg(path: &Path) -> io::Result<(usize, usize, Vec<[u8; 3]>)> {
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    let data = fs::read(path)?;
    let opts = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGB)
        .set_max_width(50_000)
        .set_max_height(50_000);
    let mut d = zune_jpeg::JpegDecoder::new_with_options(Cursor::new(data), opts);
    d.decode_headers()
        .map_err(|e| io::Error::other(format!("{e:?}")))?;
    let (w, h) = d
        .dimensions()
        .ok_or_else(|| io::Error::other("no dimensions"))?;
    too_big(w as u64, h as u64)?;
    let bytes = d.decode().map_err(|e| io::Error::other(format!("{e:?}")))?;
    let px = bytes
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    Ok((w, h, px))
}

/// Average each block of source pixels into one: a box filter, which keeps fine
/// detail as colour rather than dropping it the way picking one pixel would.
fn shrink(w: usize, h: usize, px: &[[u8; 3]], max_w: usize, max_h: usize) -> Picture {
    let scale = (max_w as f64 / w.max(1) as f64)
        .min(max_h as f64 / h.max(1) as f64)
        .min(1.0);
    let (tw, th) = (
        ((w as f64 * scale).round() as usize).max(1),
        ((h as f64 * scale).round() as usize).max(1),
    );
    let mut out = Vec::with_capacity(tw * th);
    for ty in 0..th {
        let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1));
        for tx in 0..tw {
            let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1));
            let mut sum = [0u64; 3];
            let mut n = 0u64;
            for y in y0..y1.min(h) {
                for p in px.get(y * w + x0..y * w + x1.min(w)).unwrap_or(&[]) {
                    for c in 0..3 {
                        sum[c] += p[c] as u64;
                    }
                    n += 1;
                }
            }
            let n = n.max(1);
            out.push([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]);
        }
    }
    Picture {
        width: tw,
        height: th,
        source: (w, h),
        rgb: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(path: &Path, w: u32, h: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) {
        let f = fs::File::create(path).unwrap();
        let mut e = png::Encoder::new(f, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().unwrap();
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&pixel(x, y));
            }
        }
        wr.write_image_data(&data).unwrap();
    }

    #[test]
    fn a_png_is_shrunk_keeping_its_shape_and_colours() {
        let d = std::env::temp_dir().join(format!("xdevcmd-img-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let p = d.join("half.png");
        // Left half red, right half blue, 200×100.
        write_png(&p, 200, 100, |x, _| {
            if x < 100 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        });
        let pic = load(&p, 40, 40, [0, 0, 0]).unwrap();
        assert_eq!((pic.width, pic.height), (40, 20), "2:1 kept");
        assert_eq!(pic.source, (200, 100));
        assert_eq!(pic.rgb[0], [255, 0, 0]);
        assert_eq!(pic.rgb[39], [0, 0, 255]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn transparency_is_laid_over_the_background() {
        let d = std::env::temp_dir().join(format!("xdevcmd-img-a-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let p = d.join("clear.png");
        write_png(&p, 2, 2, |_, _| [255, 255, 255, 0]);
        let pic = load(&p, 10, 10, [10, 20, 30]).unwrap();
        assert_eq!(pic.rgb[0], [10, 20, 30]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_header_claiming_billions_of_pixels_is_refused_before_decoding() {
        let d = std::env::temp_dir().join(format!("xdevcmd-img-b-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let p = d.join("bomb.png");
        // A real PNG header saying 60000×60000; the data never matters.
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(b"IHDR");
        ihdr.extend_from_slice(&60_000u32.to_be_bytes());
        ihdr.extend_from_slice(&60_000u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(&ihdr);
        bytes.extend_from_slice(&crc(&ihdr).to_be_bytes());
        // An empty IDAT: the decoder stops reading the header there.
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(b"IDAT");
        bytes.extend_from_slice(&crc(b"IDAT").to_be_bytes());
        fs::write(&p, bytes).unwrap();
        let e = load(&p, 10, 10, [0, 0, 0]).unwrap_err();
        assert!(e.to_string().contains("too large"), "{e}");
        assert!(load(&d.join("missing.jpg"), 10, 10, [0; 3]).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    fn crc(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    (c >> 1) ^ 0xEDB8_8320
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
}
