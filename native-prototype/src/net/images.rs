//! Server-supplied images (Stage 22a, brief §22, §61): the URL allow-list, the bounded fetch
//! request and off-UI-thread decoding.
//!
//! The Worker builds every image URL itself from its own origin (`skribblDrawingUrl`,
//! `profileAvatarUrl`): `${origin}/skribbl/drawing/<key>` and `${origin}/profile/avatar/<key>`.
//! So a URL is accepted only if it is exactly that shape for the **configured** API origin:
//!
//! - same scheme as the endpoint (HTTPS in production; plain HTTP only for the loopback test
//!   endpoint), same host (ASCII case-insensitive), same port;
//! - no user info, query or fragment; no `..`, backslashes, whitespace or control characters;
//! - path under the route the image kind expects, at most 512 bytes.
//!
//! The fetch never carries credentials (the image routes are unauthenticated), never follows a
//! redirect (transport), is size-bounded per kind, and is decoded only if the declared content
//! type *and* the file signature agree on PNG, WebP or JPEG, under decoder memory limits.

use std::time::Duration;

use super::endpoint::Origin;
use super::http::{ApiRequest, Body, Method, NetError, Priority, Target};

/// Which server image route a URL must belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageKind {
    SkribblDrawing,
    Avatar,
}

impl ImageKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::SkribblDrawing => "/skribbl/drawing/",
            Self::Avatar => "/profile/avatar/",
        }
    }

    /// The Worker's upload caps (`MAX_SKRIBBL_IMAGE_BYTES` 1.5 MB, `MAX_PROFILE_AVATAR_BYTES`
    /// 256 KiB) with headroom; a response above this is refused while reading.
    pub fn max_bytes(self) -> u64 {
        match self {
            Self::SkribblDrawing => 2 * 1024 * 1024,
            Self::Avatar => 512 * 1024,
        }
    }

    fn accepts(self, format: ImageFormat) -> bool {
        match self {
            // `skribblImageTypes`: PNG or WebP
            Self::SkribblDrawing => matches!(format, ImageFormat::Png | ImageFormat::WebP),
            Self::Avatar => true,
        }
    }
}

pub const MAX_IMAGE_URL_LEN: usize = 512;
const IMAGE_TIMEOUT: Duration = Duration::from_secs(20);
/// Decoder limits: no server image is larger than the 900x600 canvas or a 512 px avatar.
const MAX_IMAGE_DIMENSION: u32 = 4096;
const MAX_DECODE_ALLOC: u64 = 64 * 1024 * 1024;

/// A path that passed the policy (what the transport appends to the configured origin).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImagePath(String);

impl ImagePath {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn path_char_ok(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-._~!$&'()*+,;=:@/%".contains(c)
}

/// Applies the policy. `None` for anything that is not the configured origin's image route.
pub fn allow(url: &str, kind: ImageKind, origin: &Origin) -> Option<ImagePath> {
    if url.len() > MAX_IMAGE_URL_LEN || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let rest = if origin.secure {
        url.strip_prefix("https://")?
    } else {
        // plain HTTP exists only for the loopback test endpoint
        if !origin.is_loopback() {
            return None;
        }
        url.strip_prefix("http://")?
    };
    let slash = rest.find('/')?;
    let (authority, path) = rest.split_at(slash);
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let default_port = if origin.secure { 443 } else { 80 };
    let (host, port) = if authority.starts_with('[') {
        // an IPv6 literal: `[::1]` or `[::1]:port`
        let end = authority.find(']')?;
        let (h, rest) = authority.split_at(end + 1);
        match rest.strip_prefix(':') {
            Some(p) => (h, p.parse::<u16>().ok()?),
            None if rest.is_empty() => (h, default_port),
            None => return None,
        }
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h, p.parse::<u16>().ok()?),
            None => (authority, default_port),
        }
    };
    if !host.eq_ignore_ascii_case(&origin.host) || port != origin.port {
        return None;
    }
    if path.contains(['?', '#', '\\'])
        || !path.starts_with(kind.prefix())
        || path.len() == kind.prefix().len()
        || !path.chars().all(path_char_ok)
        || path.split('/').any(|seg| seg == ".." || seg == ".")
    {
        return None;
    }
    // a percent-encoded traversal (`%2e%2e`) or slash is fine inside the key: the Worker decodes
    // the whole remainder as one R2 key; it cannot leave the route prefix
    Some(ImagePath(path.to_string()))
}

/// The GET for an allowed image: no query, no body, no credentials, its own size cap.
pub fn request(path: &ImagePath, kind: ImageKind) -> ApiRequest {
    ApiRequest {
        method: Method::Get,
        target: Target::Image(path.0.clone()),
        query: Vec::new(),
        body: Body::None,
        max_response: kind.max_bytes(),
        timeout: IMAGE_TIMEOUT,
        priority: Priority::Image,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    WebP,
    Jpeg,
}

fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some(ImageFormat::Png)
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageFormat::WebP)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(ImageFormat::Jpeg)
    } else {
        None
    }
}

fn declared(content_type: Option<&str>) -> Option<ImageFormat> {
    let ct = content_type?.split(';').next()?.trim().to_ascii_lowercase();
    match ct.as_str() {
        "image/png" => Some(ImageFormat::Png),
        "image/webp" => Some(ImageFormat::WebP),
        "image/jpeg" | "image/jpg" => Some(ImageFormat::Jpeg),
        _ => None,
    }
}

/// A decoded, downscaled RGBA image, ready to become a `slint::Image` on the UI thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl DecodedImage {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn byte_size(&self) -> usize {
        self.rgba.len()
    }
}

/// Checks type and signature, decodes under limits and scales down to fit `max_w`x`max_h`
/// (aspect kept; never up-scaled). Runs on the network thread.
pub fn decode(
    kind: ImageKind,
    content_type: Option<&str>,
    bytes: &[u8],
    max_w: u32,
    max_h: u32,
) -> Result<DecodedImage, NetError> {
    let sniffed = sniff(bytes).ok_or(NetError::Malformed)?;
    // the Worker always sends the stored mime type; a mismatch is not an image we trust
    if declared(content_type) != Some(sniffed) || !kind.accepts(sniffed) {
        return Err(NetError::Malformed);
    }
    let format = match sniffed {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::WebP => image::ImageFormat::WebP,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
    };
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|_| NetError::Malformed)?;
    let rgba = decoded.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return Err(NetError::Malformed);
    }
    let scale = (f64::from(max_w) / f64::from(w))
        .min(f64::from(max_h) / f64::from(h))
        .min(1.0);
    let rgba = if scale < 1.0 {
        let tw = ((f64::from(w) * scale).round() as u32).max(1);
        let th = ((f64::from(h) * scale).round() as u32).max(1);
        image::imageops::thumbnail(&rgba, tw, th)
    } else {
        rgba
    };
    let (width, height) = rgba.dimensions();
    Ok(DecodedImage {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prod() -> Origin {
        Origin {
            secure: true,
            host: "study-tracker-social.danil-poluyanov13.workers.dev".into(),
            port: 443,
        }
    }

    fn local() -> Origin {
        Origin {
            secure: false,
            host: "127.0.0.1".into(),
            port: 47811,
        }
    }

    const P: &str = "https://study-tracker-social.danil-poluyanov13.workers.dev";

    #[test]
    fn the_workers_own_image_urls_are_allowed() {
        let d = allow(
            &format!("{P}/skribbl/drawing/drawings%2F2026-10-04%2Fu1.webp"),
            ImageKind::SkribblDrawing,
            &prod(),
        )
        .unwrap();
        assert_eq!(
            d.as_str(),
            "/skribbl/drawing/drawings%2F2026-10-04%2Fu1.webp"
        );
        assert!(allow(
            &format!("{P}/profile/avatar/avatars%2Fu1%2F1.webp"),
            ImageKind::Avatar,
            &prod()
        )
        .is_some());
        assert!(
            allow(
                &format!("{}:443/skribbl/drawing/x", P),
                ImageKind::SkribblDrawing,
                &prod()
            )
            .is_some(),
            "explicit default port"
        );
        assert!(allow(
            "https://STUDY-TRACKER-SOCIAL.danil-poluyanov13.workers.dev/skribbl/drawing/x",
            ImageKind::SkribblDrawing,
            &prod()
        )
        .is_some());
        assert!(
            allow(
                "http://127.0.0.1:47811/skribbl/drawing/x.png",
                ImageKind::SkribblDrawing,
                &local()
            )
            .is_some(),
            "test mode: its own loopback origin"
        );
    }

    #[test]
    fn everything_else_is_rejected() {
        let cases = [
            format!("http://study-tracker-social.danil-poluyanov13.workers.dev/skribbl/drawing/x"),
            "file:///etc/passwd".to_string(),
            "https://127.0.0.1/skribbl/drawing/x".to_string(),
            "https://localhost/skribbl/drawing/x".to_string(),
            "https://evil.example/skribbl/drawing/x".to_string(),
            "https://study-tracker-social.danil-poluyanov13.workers.dev.evil.example/skribbl/drawing/x".to_string(),
            format!("https://user:pass@study-tracker-social.danil-poluyanov13.workers.dev/skribbl/drawing/x"),
            format!("https://x@study-tracker-social.danil-poluyanov13.workers.dev/skribbl/drawing/x"),
            format!("{P}:8443/skribbl/drawing/x"),
            format!("{P}/feed/image/x"),
            format!("{P}/skribbl/drawing/"),
            format!("{P}/skribbl/drawings/x"),
            format!("{P}/profile/avatar/x"),
            format!("{P}/skribbl/drawing/../../admin"),
            format!("{P}/skribbl/drawing/x?deviceSecret=1"),
            format!("{P}/skribbl/drawing/x#frag"),
            format!("{P}/skribbl/drawing/x y"),
            format!("{P}/skribbl/drawing/x\\..\\y"),
            format!("{P}/skribbl/drawing/{}", "a".repeat(600)),
            "javascript:alert(1)".to_string(),
            "data:image/png;base64,AAAA".to_string(),
            format!("{P}"),
            String::new(),
        ];
        for url in &cases {
            assert_eq!(
                allow(url, ImageKind::SkribblDrawing, &prod()),
                None,
                "{url}"
            );
        }
        // a localhost URL supplied by the server while in production mode
        assert_eq!(
            allow(
                "http://127.0.0.1:47811/skribbl/drawing/x",
                ImageKind::SkribblDrawing,
                &prod()
            ),
            None
        );
        // test mode: another loopback port, https, or a remote host
        for url in [
            "http://127.0.0.1:1/skribbl/drawing/x",
            "https://127.0.0.1:47811/skribbl/drawing/x",
            "http://example.com:47811/skribbl/drawing/x",
            &format!("{P}/skribbl/drawing/x"),
        ] {
            assert_eq!(
                allow(url, ImageKind::SkribblDrawing, &local()),
                None,
                "{url}"
            );
        }
    }

    #[test]
    fn image_requests_carry_no_credentials_and_are_bounded() {
        let path = allow(
            &format!("{P}/skribbl/drawing/x"),
            ImageKind::SkribblDrawing,
            &prod(),
        )
        .unwrap();
        let r = request(&path, ImageKind::SkribblDrawing);
        assert!(r.query.is_empty() && r.body.len() == 0);
        assert_eq!(r.max_response, 2 * 1024 * 1024);
        assert_eq!(r.priority, Priority::Image);
        assert_eq!(request(&path, ImageKind::Avatar).max_response, 512 * 1024);
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        wr.write_image_data(&vec![200u8; (w * h * 4) as usize])
            .unwrap();
        drop(wr);
        out
    }

    #[test]
    fn decoding_checks_type_and_signature_and_downscales() {
        let bytes = png(900, 600);
        let img = decode(
            ImageKind::SkribblDrawing,
            Some("image/png"),
            &bytes,
            360,
            240,
        )
        .unwrap();
        assert_eq!((img.width, img.height), (360, 240));
        assert_eq!(img.byte_size(), 360 * 240 * 4);
        let small = decode(
            ImageKind::SkribblDrawing,
            Some("image/png"),
            &png(10, 10),
            360,
            240,
        )
        .unwrap();
        assert_eq!((small.width, small.height), (10, 10), "never up-scaled");
        // declared type mismatch, non-image, truncated, JPEG for a drawing, empty
        assert_eq!(
            decode(ImageKind::SkribblDrawing, Some("image/webp"), &bytes, 1, 1),
            Err(NetError::Malformed)
        );
        assert_eq!(
            decode(
                ImageKind::SkribblDrawing,
                Some("text/html"),
                b"<html>",
                1,
                1
            ),
            Err(NetError::Malformed)
        );
        assert_eq!(
            decode(
                ImageKind::SkribblDrawing,
                Some("image/png"),
                &bytes[..40],
                1,
                1
            ),
            Err(NetError::Malformed)
        );
        assert_eq!(
            decode(
                ImageKind::SkribblDrawing,
                Some("image/jpeg"),
                &[0xff, 0xd8, 0xff, 0xe0],
                1,
                1
            ),
            Err(NetError::Malformed)
        );
        assert_eq!(
            decode(ImageKind::Avatar, None, &bytes, 1, 1),
            Err(NetError::Malformed)
        );
        assert_eq!(
            decode(ImageKind::Avatar, Some("image/png"), &[], 1, 1),
            Err(NetError::Malformed)
        );
    }

    #[test]
    fn decompression_bombs_are_refused_by_the_limits() {
        // a tiny PNG that claims 20000 x 20000 pixels
        let bytes = png(1, 1);
        let mut bomb = bytes.clone();
        // IHDR width/height live at bytes 16..24; patch them (the CRC will no longer match, and
        // the decoder must refuse either way without allocating gigabytes)
        bomb[16..20].copy_from_slice(&20_000u32.to_be_bytes());
        bomb[20..24].copy_from_slice(&20_000u32.to_be_bytes());
        assert_eq!(
            decode(
                ImageKind::SkribblDrawing,
                Some("image/png"),
                &bomb,
                360,
                240
            ),
            Err(NetError::Malformed)
        );
    }
}
