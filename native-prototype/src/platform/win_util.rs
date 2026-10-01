//! Small Win32 helpers shared by the Windows platform adapters.

/// UTF-16, NUL-terminated.
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Copies `text` into a fixed UTF-16 buffer (always NUL-terminated, truncated if needed).
pub fn copy_to_fixed<const N: usize>(text: &str, out: &mut [u16; N]) {
    let mut i = 0;
    for unit in text.encode_utf16() {
        if i >= N - 1 {
            break;
        }
        out[i] = unit;
        i += 1;
    }
    out[i] = 0;
}

/// FNV-1a over bytes: a stable, dependency-free 64-bit hash used to derive per-profile names.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Whether the Windows display (UI) language's default short time format is 12-hour (`h`) rather
/// than 24-hour (`H`) - the closest native equivalent of Chromium's locale-derived `hour12`, which
/// production's `displayTime` uses (Stage 19). Falls back to 24-hour if anything fails.
pub fn ui_locale_prefers_twelve_hour() -> bool {
    use windows_sys::Win32::Globalization::{
        GetLocaleInfoEx, GetUserDefaultUILanguage, LCIDToLocaleName, LOCALE_STIMEFORMAT,
    };
    let mut name = [0u16; 85];
    // SAFETY: plain queries writing into correctly sized stack buffers.
    unsafe {
        let lang = u32::from(GetUserDefaultUILanguage());
        if LCIDToLocaleName(lang, name.as_mut_ptr(), name.len() as i32, 0) == 0 {
            return false;
        }
        let mut format = [0u16; 80];
        let len = GetLocaleInfoEx(
            name.as_ptr(),
            LOCALE_STIMEFORMAT,
            format.as_mut_ptr(),
            format.len() as i32,
        );
        if len <= 0 {
            return false;
        }
        String::from_utf16_lossy(&format[..(len as usize).saturating_sub(1)]).contains('h')
    }
}
