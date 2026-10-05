//! Profile avatars (Stage 22a): production's `SocialAvatar` union, its normalisation
//! (`normalizeAvatar` in `storage.ts`, `cleanAvatar` in the Worker) and the presentation helpers
//! `ArenaAvatar` uses (`getAvatarDisplayName`, `getArenaHue`, `getInitials`).

use serde::{Deserialize, Serialize};

use super::limits::{MAX_AVATAR_NAME_LEN, MAX_AVATAR_URL_LEN};

/// `avatarStyles` (App.tsx / Worker), in production's picker order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AvatarStyle {
    Classic,
    Serif,
    Cursive,
    Graffiti,
    Pixel,
    Mono,
}

impl AvatarStyle {
    pub const ALL: [Self; 6] = [
        Self::Classic,
        Self::Serif,
        Self::Cursive,
        Self::Graffiti,
        Self::Pixel,
        Self::Mono,
    ];

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == raw)
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Serif => "serif",
            Self::Cursive => "cursive",
            Self::Graffiti => "graffiti",
            Self::Pixel => "pixel",
            Self::Mono => "mono",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Serif => "Serif",
            Self::Cursive => "Cursive",
            Self::Graffiti => "Graffiti",
            Self::Pixel => "Pixel",
            Self::Mono => "Mono",
        }
    }
}

/// `avatarIcons` (App.tsx and the Worker's identical set).
pub const AVATAR_ICONS: [&str; 40] = [
    "✦", "★", "◆", "☘", "☾", "☀", "♜", "♞", "⚡", "☕", "📚", "🧠", "🔥", "🌊", "🌿", "🪐", "🚀",
    "🎯", "🏆", "🛡", "🦉", "🐢", "🐺", "🐱", "🍄", "🌙", "🌸", "🍀", "💎", "🎲", "🎧", "📝", "🔮",
    "🧩", "🕹", "📖", "🧪", "🛰", "🌌", "🦊",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Avatar {
    Letter {
        letter: String,
        style: AvatarStyle,
    },
    Icon {
        icon: String,
    },
    /// `url` is either the Worker's `https://<origin>/profile/avatar/<key>` or (own avatar, never
    /// uploaded) a `data:image/` URI. Whether a URL may actually be fetched is the application's
    /// image policy, not this type's.
    Photo {
        name: String,
        url: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

/// `firstAvatarLetter`: the first character of the trimmed name, upper-cased, or "S".
pub fn first_avatar_letter(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map_or_else(|| "S".to_string(), |c| c.to_uppercase().collect())
}

/// `getInitials`: first letters of the name's `[\s_-]+`-separated parts, at most two, upper-cased,
/// or "ST".
pub fn initials(name: &str) -> String {
    let joined: String = name
        .trim()
        .split(|c: char| c.is_whitespace() || c == '_' || c == '-')
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect();
    let upper = joined.to_uppercase();
    if upper.is_empty() {
        "ST".to_string()
    } else {
        upper
    }
}

/// `getArenaHue`: the sum of every character's first UTF-16 code unit, mod 360. (`[...name]`
/// iterates code points and `charCodeAt(0)` reads the first unit, i.e. the high surrogate of an
/// astral character.)
pub fn arena_hue(name: &str) -> u32 {
    (arena_hue_units(name) % 360) as u32
}

/// `[...text].reduce((sum, ch) => sum + ch.charCodeAt(0), 0)`: production's string "hash" (also
/// `pickFeedFallbackNote`).
pub fn arena_hue_units(text: &str) -> u64 {
    text.chars()
        .map(|c| {
            let mut units = [0u16; 2];
            u64::from(c.encode_utf16(&mut units)[0])
        })
        .sum()
}

fn is_letter_a_to_z(raw: &str) -> bool {
    let mut chars = raw.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_ascii_alphabetic())
}

fn is_allowed_photo_url(url: &str) -> bool {
    if url.len() > MAX_AVATAR_URL_LEN {
        return false;
    }
    if url.starts_with("data:image/") {
        return true;
    }
    // /^https:\/\/[^/]+\/profile\/avatar\//, plus plain-HTTP loopback for the local test server
    // (the app's image policy still binds every URL to the configured origin, and the production
    // origin is HTTPS, so a loopback URL can never be fetched against production)
    let rest = url.strip_prefix("https://").or_else(|| {
        url.strip_prefix("http://").filter(|r| {
            [
                "127.0.0.1:",
                "127.0.0.1/",
                "localhost:",
                "localhost/",
                "[::1]:",
                "[::1]/",
            ]
            .iter()
            .any(|p| r.starts_with(p))
        })
    });
    rest.and_then(|rest| rest.split_once('/'))
        .is_some_and(|(host, path)| !host.is_empty() && path.starts_with("profile/avatar/"))
}

impl Avatar {
    /// The default for a name (`{ kind: "letter", letter: firstAvatarLetter(name), style: "classic" }`).
    pub fn default_for(display_name: &str) -> Self {
        Self::Letter {
            letter: first_avatar_letter(display_name),
            style: AvatarStyle::Classic,
        }
    }

    /// `normalizeAvatar`/`cleanAvatar`: an icon from the set, a single A-Z letter (upper-cased)
    /// with a known style (else classic), a photo with a Worker or data: URL; anything else
    /// falls back to the default letter avatar.
    pub fn normalized(
        kind: Option<&str>,
        letter: Option<&str>,
        style: Option<&str>,
        icon: Option<&str>,
        photo: Option<(&str, &str, &str)>,
        display_name: &str,
    ) -> Self {
        match kind {
            Some("icon") => {
                if let Some(icon) = icon.filter(|i| AVATAR_ICONS.contains(i)) {
                    return Self::Icon {
                        icon: icon.to_string(),
                    };
                }
            }
            Some("letter") => {
                let letter = letter
                    .filter(|l| is_letter_a_to_z(l))
                    .map_or_else(|| first_avatar_letter(display_name), str::to_uppercase);
                let style = style
                    .and_then(AvatarStyle::parse)
                    .unwrap_or(AvatarStyle::Classic);
                return Self::Letter { letter, style };
            }
            Some("photo") => {
                if let Some((name, url, mime)) =
                    photo.filter(|(_, url, _)| is_allowed_photo_url(url))
                {
                    return Self::Photo {
                        name: name.chars().take(MAX_AVATAR_NAME_LEN).collect(),
                        url: url.to_string(),
                        mime_type: if mime.starts_with("image/") {
                            mime.to_string()
                        } else {
                            "image/webp".to_string()
                        },
                    };
                }
            }
            _ => {}
        }
        Self::default_for(display_name)
    }

    /// `getAvatarDisplayName`: the glyph(s) drawn when no photo is shown.
    pub fn display_text(&self, name: &str) -> String {
        match self {
            Self::Icon { icon } => icon.clone(),
            Self::Photo { url, .. } if !url.is_empty() => String::new(),
            Self::Photo { .. } => initials(name),
            Self::Letter { letter, .. } if !letter.is_empty() => letter.clone(),
            Self::Letter { .. } => first_avatar_letter(name),
        }
    }

    /// The remote photo URL to fetch, if any (never a `data:` URI). Only a candidate: the app's
    /// image policy still checks scheme, origin, path, size and type (plain `http://` passes it
    /// only for the loopback test endpoint).
    pub fn remote_photo_url(&self) -> Option<&str> {
        match self {
            Self::Photo { url, .. }
                if url.starts_with("https://") || url.starts_with("http://") =>
            {
                Some(url)
            }
            _ => None,
        }
    }
}

/// The profile photo crop editor (Stage 22b): production's `AVATAR_CROP_*` constants and
/// `clampAvatarCrop` / pointer / wheel / slider / `cropAvatarToDataUrl` geometry, in CSS pixels
/// of the 300 px square stage. `x`/`y` are the crop centre as fractions of the source image.
pub mod crop {
    /// `AVATAR_CROP_VIEWPORT_PX`.
    pub const VIEWPORT: f64 = 300.0;
    /// `AVATAR_CROP_MAX_ZOOM`.
    pub const MAX_ZOOM: f64 = 4.0;
    /// The wheel's zoom step (`deltaY < 0 ? 1.08 : 1 / 1.08`).
    pub const WHEEL_FACTOR: f64 = 1.08;
    /// `AVATAR_SOURCE_IMAGE_MAX_BYTES`: the picked file.
    pub const SOURCE_MAX_BYTES: u64 = 1024 * 1024;
    /// `AVATAR_IMAGE_MAX_BYTES` / `AVATAR_IMAGE_MAX_DIMENSION`: the cropped result.
    pub const RESULT_MAX_BYTES: usize = 96 * 1024;
    pub const RESULT_DIMENSION: u32 = 160;
    /// `AVATAR_IMAGE_COMPRESSION_ATTEMPTS`: (side, quality).
    pub const ATTEMPTS: [(u32, f32); 4] = [(160, 0.72), (160, 0.58), (160, 0.45), (128, 0.45)];

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Crop {
        pub x: f64,
        pub y: f64,
        pub zoom: f64,
    }

    impl Default for Crop {
        /// `{ x: 0.5, y: 0.5, zoom: 1 }` when a photo is picked.
        fn default() -> Self {
            Self {
                x: 0.5,
                y: 0.5,
                zoom: 1.0,
            }
        }
    }

    /// The displayed scale: the image covers the stage at zoom 1.
    pub fn scale(w: f64, h: f64, zoom: f64) -> f64 {
        (VIEWPORT / w).max(VIEWPORT / h) * zoom
    }

    /// `clampAvatarCrop`: the stage never shows outside the image.
    pub fn clamp(c: Crop, w: f64, h: f64) -> Crop {
        if !(w > 0.0 && h > 0.0) {
            return Crop::default();
        }
        let zoom = c.zoom.clamp(1.0, MAX_ZOOM);
        let s = scale(w, h, zoom);
        let half_w = VIEWPORT / (s * 2.0 * w);
        let half_h = VIEWPORT / (s * 2.0 * h);
        Crop {
            x: c.x.max(half_w).min(1.0 - half_w),
            y: c.y.max(half_h).min(1.0 - half_h),
            zoom,
        }
    }

    /// `handleAvatarCropPointerMove`: the image follows the pointer from the drag's start.
    pub fn dragged(start: Crop, dx: f64, dy: f64, w: f64, h: f64) -> Crop {
        let s = scale(w, h, start.zoom);
        clamp(
            Crop {
                x: start.x - dx / (w * s),
                y: start.y - dy / (h * s),
                zoom: start.zoom,
            },
            w,
            h,
        )
    }

    /// `handleAvatarCropWheel`.
    pub fn wheeled(c: Crop, zoom_in: bool, w: f64, h: f64) -> Crop {
        let f = if zoom_in {
            WHEEL_FACTOR
        } else {
            1.0 / WHEEL_FACTOR
        };
        clamp(
            Crop {
                zoom: (c.zoom * f).clamp(1.0, MAX_ZOOM),
                ..c
            },
            w,
            h,
        )
    }

    /// `handleAvatarCropZoomChange` (the slider).
    pub fn zoomed(c: Crop, value: f64, w: f64, h: f64) -> Crop {
        clamp(
            Crop {
                zoom: value.clamp(1.0, MAX_ZOOM),
                ..c
            },
            w,
            h,
        )
    }

    /// Where the image is drawn on the stage: `(left, top, width, height)`.
    pub fn image_rect(c: Crop, w: f64, h: f64) -> (f64, f64, f64, f64) {
        let s = scale(w, h, c.zoom);
        (
            VIEWPORT / 2.0 - c.x * w * s,
            VIEWPORT / 2.0 - c.y * h * s,
            w * s,
            h * s,
        )
    }

    /// The source square `cropAvatarToDataUrl` draws: `(sx, sy, side)` in source pixels.
    pub fn source_square(c: Crop, w: f64, h: f64) -> (f64, f64, f64) {
        let side = VIEWPORT / scale(w, h, c.zoom);
        (c.x * w - side / 2.0, c.y * h - side / 2.0, side)
    }
}
