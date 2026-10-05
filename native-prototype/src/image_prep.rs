//! Picked images (Stage 22b): production's `prepareFeedImage`, `handleProfileAvatarPhotoChange`
//! and `cropAvatarToDataUrl`, natively and bounded. Runs on the network worker thread
//! (`app_net::submit_local`), never on the UI thread.
//!
//! - The file's **bytes** decide its type (the extension and name are not trusted); production's
//!   accepted set: PNG, JPEG, WebP, GIF.
//! - Size limits are production's (5 MB feed image, 1 MB avatar photo), checked from the file's
//!   metadata *before* reading it, and the read itself is capped.
//! - Decoding uses the shared decoder limits (dimensions and allocation), so a decompression bomb
//!   is refused instead of allocated.
//! - Production encodes WebP (`canvas.toBlob("image/webp", 0.82)`); the image crate has no lossy
//!   WebP encoder, so native encodes **JPEG** (quality 82) for opaque pictures and **PNG** for
//!   pictures with transparency. The Worker accepts both; the difference is the byte format only
//!   (parity-debt register: implementation detail). A GIF is uploaded as picked, like production.
//! - Error texts are production's.
//! - A local path is never logged or sent anywhere; only the file *name* travels (as production's
//!   `File.name` does for the avatar upload).

use std::io::Read;
use std::path::Path;

use study_tracker_core::social::avatar::crop::{self, Crop};

use crate::net::images::{decode_bytes, sniff_format, DecodedImage, ImageFormat};
use crate::net::social_ext::Upload;
use crate::social_controller::avatar::CropSource;
use crate::social_controller::feed::PreparedImage;

/// `FEED_IMAGE_MAX_BYTES`.
pub const FEED_MAX_BYTES: u64 = 5 * 1024 * 1024;
/// `FEED_IMAGE_MAX_DIMENSION`.
pub const FEED_MAX_DIMENSION: u32 = 1280;
/// The crop stage works on at most this many pixels per side (the result is 160 px).
const CROP_SOURCE_MAX: u32 = 2048;
/// Composer / editor preview.
const PREVIEW_MAX: u32 = 360;

fn read_capped(path: &Path, max: u64) -> Result<Vec<u8>, ()> {
    let mut f = std::fs::File::open(path).map_err(|_| ())?;
    let mut buf = Vec::new();
    f.by_ref()
        .take(max + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ())?;
    Ok(buf)
}

fn has_alpha(img: &DecodedImage) -> bool {
    img.rgba.chunks_exact(4).any(|p| p[3] < 255)
}

pub fn encode_png(img: &DecodedImage) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(&img.rgba).ok()?;
    }
    Some(out)
}

pub fn encode_jpeg(img: &DecodedImage, quality: u8) -> Option<Vec<u8>> {
    let rgb: Vec<u8> = img
        .rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let mut out = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    enc.encode(&rgb, img.width, img.height, image::ExtendedColorType::Rgb8)
        .ok()?;
    Some(out)
}

fn scale_to(img: DecodedImage, max: u32) -> DecodedImage {
    if img.width <= max && img.height <= max {
        return img;
    }
    let s = (f64::from(max) / f64::from(img.width.max(img.height))).min(1.0);
    let w = ((f64::from(img.width) * s).round() as u32).max(1);
    let h = ((f64::from(img.height) * s).round() as u32).max(1);
    let Some(buf) = image::RgbaImage::from_raw(img.width, img.height, img.rgba) else {
        return DecodedImage {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        };
    };
    let out = image::imageops::resize(&buf, w, h, image::imageops::FilterType::Triangle);
    DecodedImage {
        width: w,
        height: h,
        rgba: out.into_raw(),
    }
}

/// `prepareFeedImage(file)`.
pub fn prepare_feed_image(path: &Path) -> Result<PreparedImage, String> {
    let len = std::fs::metadata(path)
        .map_err(|_| "Could not prepare image.".to_string())?
        .len();
    if len > FEED_MAX_BYTES {
        return Err("Image is too large. Use an image under 5 MB.".into());
    }
    let bytes =
        read_capped(path, FEED_MAX_BYTES).map_err(|_| "Could not prepare image.".to_string())?;
    if bytes.len() as u64 > FEED_MAX_BYTES {
        return Err("Image is too large. Use an image under 5 MB.".into());
    }
    let Some(format) = sniff_format(&bytes) else {
        return Err("Use PNG, JPEG, WebP, or GIF images.".into());
    };
    // a GIF goes up as picked (animation included); its first frame is the preview
    if format == ImageFormat::Gif {
        let preview = decode_bytes(format, &bytes, PREVIEW_MAX, PREVIEW_MAX)
            .map_err(|_| "Could not prepare image.".to_string())?;
        return Ok(PreparedImage {
            upload: Upload {
                bytes,
                mime: "image/gif",
            },
            preview,
        });
    }
    let full = decode_bytes(format, &bytes, FEED_MAX_DIMENSION, FEED_MAX_DIMENSION)
        .map_err(|_| "Could not prepare image.".to_string())?;
    let (encoded, mime) = if has_alpha(&full) {
        (encode_png(&full), "image/png")
    } else {
        (encode_jpeg(&full, 82), "image/jpeg")
    };
    let encoded = encoded.ok_or_else(|| "Could not prepare image.".to_string())?;
    if encoded.len() as u64 > FEED_MAX_BYTES {
        return Err("Image is still too large after compression.".into());
    }
    let preview = scale_to(full, PREVIEW_MAX);
    Ok(PreparedImage {
        upload: Upload {
            bytes: encoded,
            mime,
        },
        preview,
    })
}

/// `handleProfileAvatarPhotoChange`: the photo for the crop stage.
pub fn load_avatar_source(path: &Path) -> Result<CropSource, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.chars().take(180).collect::<String>())
        .unwrap_or_else(|| "photo".into());
    let len = std::fs::metadata(path)
        .map_err(|_| "Could not prepare photo.".to_string())?
        .len();
    if len > crop::SOURCE_MAX_BYTES {
        return Err("Photo is too large. Use an image under 1 MB.".into());
    }
    let bytes = read_capped(path, crop::SOURCE_MAX_BYTES)
        .map_err(|_| "Could not prepare photo.".to_string())?;
    let Some(format) = sniff_format(&bytes) else {
        return Err("Choose an image file.".into());
    };
    let image = decode_bytes(format, &bytes, CROP_SOURCE_MAX, CROP_SOURCE_MAX)
        .map_err(|_| "Could not load photo.".to_string())?;
    Ok(CropSource { image, name })
}

/// Bilinear sample of the source square into a `side` x `side` RGBA picture (outside the
/// source is transparent, as the canvas would leave it).
fn render_square(src: &DecodedImage, sx: f64, sy: f64, size: f64, side: u32) -> DecodedImage {
    let mut out = vec![0u8; (side * side * 4) as usize];
    let (w, h) = (src.width as i64, src.height as i64);
    let px = |x: i64, y: i64| -> [f64; 4] {
        if x < 0 || y < 0 || x >= w || y >= h {
            return [0.0; 4];
        }
        let i = ((y * w + x) * 4) as usize;
        [
            f64::from(src.rgba[i]),
            f64::from(src.rgba[i + 1]),
            f64::from(src.rgba[i + 2]),
            f64::from(src.rgba[i + 3]),
        ]
    };
    let step = size / f64::from(side);
    for oy in 0..side {
        for ox in 0..side {
            let fx = sx + (f64::from(ox) + 0.5) * step - 0.5;
            let fy = sy + (f64::from(oy) + 0.5) * step - 0.5;
            let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
            let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
            let a = px(x0, y0);
            let b = px(x0 + 1, y0);
            let c = px(x0, y0 + 1);
            let d = px(x0 + 1, y0 + 1);
            let i = ((oy * side + ox) * 4) as usize;
            for k in 0..4 {
                let v = a[k] * (1.0 - tx) * (1.0 - ty)
                    + b[k] * tx * (1.0 - ty)
                    + c[k] * (1.0 - tx) * ty
                    + d[k] * tx * ty;
                out[i + k] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    DecodedImage {
        width: side,
        height: side,
        rgba: out,
    }
}

/// `cropAvatarToDataUrl`: the 160 px square (128 px as the last attempt), as small as production
/// asks (`AVATAR_IMAGE_MAX_BYTES`, 96 KB): PNG when it fits, else JPEG at production's quality
/// ladder.
pub fn render_crop(source: &CropSource, c: Crop) -> Result<PreparedImage, String> {
    let (w, h) = (
        f64::from(source.image.width),
        f64::from(source.image.height),
    );
    let (sx, sy, size) = crop::source_square(c, w, h);
    for (side, quality) in crop::ATTEMPTS {
        let pic = render_square(&source.image, sx, sy, size, side);
        if let Some(png) = encode_png(&pic) {
            if png.len() <= crop::RESULT_MAX_BYTES {
                return Ok(PreparedImage {
                    upload: Upload {
                        bytes: png,
                        mime: "image/png",
                    },
                    preview: pic,
                });
            }
        }
        if let Some(jpg) = encode_jpeg(&pic, (quality * 100.0).round() as u8) {
            if jpg.len() <= crop::RESULT_MAX_BYTES {
                return Ok(PreparedImage {
                    upload: Upload {
                        bytes: jpg,
                        mime: "image/jpeg",
                    },
                    preview: pic,
                });
            }
        }
    }
    Err("Photo is still too large after compression. Try a simpler or smaller photo.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("st22b-prep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    fn png(w: u32, h: u32, alpha: u8) -> Vec<u8> {
        encode_png(&DecodedImage {
            width: w,
            height: h,
            rgba: (0..w * h)
                .flat_map(|i| [(i % 251) as u8, 90, 160, alpha])
                .collect(),
        })
        .unwrap()
    }

    #[test]
    fn feed_images_are_validated_resized_and_encoded() {
        // a large opaque picture: resized to 1280 and sent as JPEG
        let p = tmp("big.png", &png(2000, 1000, 255));
        let prep = prepare_feed_image(&p).unwrap();
        assert_eq!(prep.upload.mime, "image/jpeg");
        assert!(prep.preview.width <= 360);
        let back = decode_bytes(ImageFormat::Jpeg, &prep.upload.bytes, 4000, 4000).unwrap();
        assert_eq!((back.width, back.height), (1280, 640));
        // transparency is kept (PNG)
        let p = tmp("alpha.png", &png(64, 64, 100));
        assert_eq!(prepare_feed_image(&p).unwrap().upload.mime, "image/png");
        // the extension lies: the bytes decide
        let p = tmp("not-an-image.png", b"<html>hello</html>");
        assert_eq!(
            prepare_feed_image(&p).unwrap_err(),
            "Use PNG, JPEG, WebP, or GIF images."
        );
        // a corrupt PNG (signature only)
        let mut bad = png(10, 10, 255);
        bad.truncate(40);
        let p = tmp("corrupt.png", &bad);
        assert_eq!(
            prepare_feed_image(&p).unwrap_err(),
            "Could not prepare image."
        );
        // too large: refused from the metadata, never read
        let p = tmp("huge.bin", &vec![0u8; (FEED_MAX_BYTES + 1) as usize]);
        assert_eq!(
            prepare_feed_image(&p).unwrap_err(),
            "Image is too large. Use an image under 5 MB."
        );
        // a decompression bomb (a tiny file claiming 60000 x 60000) is refused by the limits
        let bomb = {
            let mut v = png(1, 1, 255);
            // patch IHDR width/height (big-endian at bytes 16..24)
            v[16..20].copy_from_slice(&60000u32.to_be_bytes());
            v[20..24].copy_from_slice(&60000u32.to_be_bytes());
            v
        };
        let p = tmp("bomb.png", &bomb);
        assert!(prepare_feed_image(&p).is_err());
        assert!(prepare_feed_image(Path::new("/nonexistent/st22b")).is_err());
    }

    #[test]
    fn avatar_photos_crop_to_a_small_square() {
        let p = tmp("me.png", &png(800, 600, 255));
        let src = load_avatar_source(&p).unwrap();
        assert_eq!(src.name, "me.png");
        let c = crop::clamp(
            Crop {
                x: 0.5,
                y: 0.5,
                zoom: 1.0,
            },
            800.0,
            600.0,
        );
        let out = render_crop(&src, c).unwrap();
        assert!(out.upload.bytes.len() <= crop::RESULT_MAX_BYTES);
        assert!(out.preview.width == 160 || out.preview.width == 128);
        let p = tmp(
            "big-photo.bin",
            &vec![1u8; (crop::SOURCE_MAX_BYTES + 1) as usize],
        );
        assert_eq!(
            load_avatar_source(&p).unwrap_err(),
            "Photo is too large. Use an image under 1 MB."
        );
        let p = tmp("text.jpg", b"hello");
        assert_eq!(load_avatar_source(&p).unwrap_err(), "Choose an image file.");
    }

    #[test]
    fn crop_geometry_matches_production() {
        let (w, h) = (800.0, 600.0);
        // zoom 1 covers the stage: the short side fills 300 px
        let (_, _, dw, dh) = crop::image_rect(Crop::default(), w, h);
        assert_eq!((dw.round(), dh.round()), (400.0, 300.0));
        // the crop never leaves the image
        let c = crop::clamp(
            Crop {
                x: 0.0,
                y: 2.0,
                zoom: 9.0,
            },
            w,
            h,
        );
        assert_eq!(c.zoom, crop::MAX_ZOOM);
        let (sx, sy, side) = crop::source_square(c, w, h);
        assert!(sx >= -1e-9 && sy + side <= h + 1e-9);
        // dragging right moves the crop left
        let d = crop::dragged(Crop::default(), 50.0, 0.0, w, h);
        assert!(d.x < 0.5);
        let z = crop::wheeled(Crop::default(), true, w, h);
        assert!((z.zoom - 1.08).abs() < 1e-9);
        assert_eq!(crop::wheeled(Crop::default(), false, w, h).zoom, 1.0);
    }
}
