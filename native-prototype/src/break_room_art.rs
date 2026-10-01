//! Achievement art (Stage 20): production's 42 hand-painted pieces (`desktop/public/achievements`,
//! byte-identical copies), embedded and decoded **lazily** - a piece is decoded the first time an
//! album page that shows it is built, then kept (at most 42 small images), never at startup.

use std::cell::RefCell;
use std::collections::HashMap;

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

fn bytes(id: &str) -> Option<&'static [u8]> {
    Some(match id {
        "early-bird" => include_bytes!("../assets/break/achievements/early-bird.png"),
        "explorer" => include_bytes!("../assets/break/achievements/explorer.png"),
        "first-break" => include_bytes!("../assets/break/achievements/first-break.png"),
        "fossil-10" => include_bytes!("../assets/break/achievements/fossil-10.png"),
        "fossil-100" => include_bytes!("../assets/break/achievements/fossil-100.png"),
        "fossil-1000" => include_bytes!("../assets/break/achievements/fossil-1000.png"),
        "fossil-25" => include_bytes!("../assets/break/achievements/fossil-25.png"),
        "fossil-250" => include_bytes!("../assets/break/achievements/fossil-250.png"),
        "fossil-50" => include_bytes!("../assets/break/achievements/fossil-50.png"),
        "fossil-500" => include_bytes!("../assets/break/achievements/fossil-500.png"),
        "full-house" => include_bytes!("../assets/break/achievements/full-house.png"),
        "garden-cross-pollinator" => {
            include_bytes!("../assets/break/achievements/garden-cross-pollinator.png")
        }
        "garden-first-sprout" => {
            include_bytes!("../assets/break/achievements/garden-first-sprout.png")
        }
        "garden-full-bloom" => include_bytes!("../assets/break/achievements/garden-full-bloom.png"),
        "garden-harvest-season" => {
            include_bytes!("../assets/break/achievements/garden-harvest-season.png")
        }
        "garden-mushroom-ring" => {
            include_bytes!("../assets/break/achievements/garden-mushroom-ring.png")
        }
        "garden-streak-bloom" => {
            include_bytes!("../assets/break/achievements/garden-streak-bloom.png")
        }
        "garden-wise-tree" => include_bytes!("../assets/break/achievements/garden-wise-tree.png"),
        "night-owl" => include_bytes!("../assets/break/achievements/night-owl.png"),
        "on-fire" => include_bytes!("../assets/break/achievements/on-fire.png"),
        "perfectionist" => include_bytes!("../assets/break/achievements/perfectionist.png"),
        "rock-blooming" => include_bytes!("../assets/break/achievements/rock-blooming.png"),
        "rock-celestial" => include_bytes!("../assets/break/achievements/rock-celestial.png"),
        "rock-cosmic" => include_bytes!("../assets/break/achievements/rock-cosmic.png"),
        "rock-demon" => include_bytes!("../assets/break/achievements/rock-demon.png"),
        "rock-eternal" => include_bytes!("../assets/break/achievements/rock-eternal.png"),
        "rock-flourished" => include_bytes!("../assets/break/achievements/rock-flourished.png"),
        "rock-galactic" => include_bytes!("../assets/break/achievements/rock-galactic.png"),
        "rock-god" => include_bytes!("../assets/break/achievements/rock-god.png"),
        "rock-growing" => include_bytes!("../assets/break/achievements/rock-growing.png"),
        "rock-guardian-angel" => {
            include_bytes!("../assets/break/achievements/rock-guardian-angel.png")
        }
        "rock-heavenly" => include_bytes!("../assets/break/achievements/rock-heavenly.png"),
        "rock-hellish" => include_bytes!("../assets/break/achievements/rock-hellish.png"),
        "rock-hells-diplomat" => {
            include_bytes!("../assets/break/achievements/rock-hells-diplomat.png")
        }
        "rock-meteoric" => include_bytes!("../assets/break/achievements/rock-meteoric.png"),
        "rock-planetary" => include_bytes!("../assets/break/achievements/rock-planetary.png"),
        "rock-royal" => include_bytes!("../assets/break/achievements/rock-royal.png"),
        "rock-saint" => include_bytes!("../assets/break/achievements/rock-saint.png"),
        "rock-sprouting" => include_bytes!("../assets/break/achievements/rock-sprouting.png"),
        "rock-starstone" => include_bytes!("../assets/break/achievements/rock-starstone.png"),
        "speedrunner" => include_bytes!("../assets/break/achievements/speedrunner.png"),
        "veteran" => include_bytes!("../assets/break/achievements/veteran.png"),
        _ => return None,
    })
}

thread_local! {
    static CACHE: RefCell<HashMap<String, Image>> = RefCell::new(HashMap::new());
}

/// Decodes an 8-bit RGBA PNG (what every art piece is) into a Slint image.
pub fn decode_png_rgba(data: &[u8]) -> Option<Image> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buf[..info.buffer_size()]
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf[..info.buffer_size()]
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf[..info.buffer_size()]
            .iter()
            .flat_map(|&g| [g, g, g, 255])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Image::from_rgba8(
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&rgba, w, h),
    ))
}

/// The art for an achievement id (`achievementArt`); every id production can award has a piece.
pub fn art(id: &str) -> Image {
    CACHE.with(|cache| {
        if let Some(image) = cache.borrow().get(id) {
            return image.clone();
        }
        let image = bytes(id).and_then(decode_png_rgba).unwrap_or_default();
        cache.borrow_mut().insert(id.to_string(), image.clone());
        image
    })
}

/// Pieces decoded so far (diagnostics: bounded by 42).
pub fn decoded() -> usize {
    CACHE.with(|cache| cache.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tracker_core::break_room::achievements::has_real_art;

    #[test]
    fn every_art_piece_decodes_and_matches_productions_real_art_set() {
        let ids = [
            "early-bird",
            "explorer",
            "first-break",
            "fossil-10",
            "fossil-100",
            "fossil-1000",
            "fossil-25",
            "fossil-250",
            "fossil-50",
            "fossil-500",
            "full-house",
            "garden-cross-pollinator",
            "garden-first-sprout",
            "garden-full-bloom",
            "garden-harvest-season",
            "garden-mushroom-ring",
            "garden-streak-bloom",
            "garden-wise-tree",
            "night-owl",
            "on-fire",
            "perfectionist",
            "rock-blooming",
            "rock-celestial",
            "rock-cosmic",
            "rock-demon",
            "rock-eternal",
            "rock-flourished",
            "rock-galactic",
            "rock-god",
            "rock-growing",
            "rock-guardian-angel",
            "rock-heavenly",
            "rock-hellish",
            "rock-hells-diplomat",
            "rock-meteoric",
            "rock-planetary",
            "rock-royal",
            "rock-saint",
            "rock-sprouting",
            "rock-starstone",
            "speedrunner",
            "veteran",
        ];
        assert_eq!(ids.len(), 42);
        for id in ids {
            assert!(has_real_art(id), "{id}");
            let image = bytes(id).and_then(decode_png_rgba).expect(id);
            assert!(image.size().width > 50 && image.size().height > 50);
        }
        assert!(bytes("rock-current").is_none());
    }
}
