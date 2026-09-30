//! Authenticity: minisign (Ed25519) verification of an update artifact, compatible with what
//! Tauri's CLI signer and `tauri-plugin-updater` produce and check.
//!
//! **Trust root** = one public key, compiled into the binary ([`PRODUCTION_PUBLIC_KEY`]: the same
//! base64-encoded minisign `.pub` file production carries in `tauri.conf.json`
//! `plugins.updater.pubkey`; it is a *public* key, so committing it discloses nothing). Because it
//! is the same key production's CI signs with, the first native release can be offered to
//! already-installed Tauri builds through the existing pipeline, and the native app accepts
//! exactly the releases the maintainer signs.
//!
//! `signature` in the manifest is the base64 of the minisign `.sig` file text
//! (`untrusted comment` / signature line / `trusted comment` / global signature); verification
//! covers both the artifact bytes and the trusted comment.

use minisign_verify::{PublicKey, Signature};

use super::UpdateError;

/// `plugins.updater.pubkey` from production's `tauri.conf.json` (v0.1.67). Key id `BD4FF4F233057F12`.
pub const PRODUCTION_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEJENEZGNEYyMzMwNTdGMTIKUldRU2Z3VXo4dlJQdldobHhZdHVtUXViNTJvaXY5M01YbDg3aWRjNUc0SWtWUVp0ZUp0UUU3WWEK";

pub struct TrustRoot {
    key: PublicKey,
}

impl std::fmt::Debug for TrustRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TrustRoot(minisign public key)")
    }
}

/// Standard-alphabet base64 decoder (padding optional, ASCII whitespace ignored). Small and
/// dependency-free; used only for the public key and signature strings.
pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut padding = 0;
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                padding += 1;
                continue;
            }
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        if padding > 0 {
            return None; // data after padding
        }
        acc = (acc << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if padding > 2 {
        return None;
    }
    Some(out)
}

impl TrustRoot {
    /// From the base64 form used in `tauri.conf.json`.
    pub fn from_config_public_key(base64_pub_file: &str) -> Result<Self, UpdateError> {
        let bytes = base64_decode(base64_pub_file)
            .ok_or_else(|| UpdateError::BadTrustRoot("public key is not base64".into()))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| UpdateError::BadTrustRoot("public key is not text".into()))?;
        let key = PublicKey::decode(&text).map_err(|e| UpdateError::BadTrustRoot(e.to_string()))?;
        Ok(Self { key })
    }

    #[cfg(test)]
    pub fn production() -> Result<Self, UpdateError> {
        Self::from_config_public_key(PRODUCTION_PUBLIC_KEY)
    }

    /// Verifies `artifact` against the manifest's `signature` field. Any failure - malformed
    /// signature, signature by another key, altered artifact, altered trusted comment - is
    /// `BadSignature`; the caller must then discard the artifact.
    pub fn verify(&self, artifact: &[u8], signature_b64: &str) -> Result<(), UpdateError> {
        if signature_b64.trim().is_empty() {
            return Err(UpdateError::MissingSignature);
        }
        let bytes = base64_decode(signature_b64)
            .ok_or_else(|| UpdateError::BadSignature("signature is not base64".into()))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| UpdateError::BadSignature("signature is not text".into()))?;
        let signature =
            Signature::decode(&text).map_err(|e| UpdateError::BadSignature(e.to_string()))?;
        // `allow_legacy = true`: Tauri's signer writes classic (non-prehashed) minisign signatures;
        // both forms are real Ed25519 over the content, "legacy" refers only to the file format.
        self.key
            .verify(artifact, &signature, true)
            .map_err(|e| UpdateError::BadSignature(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = include_str!("../../../tests/fixtures/updater/test-public-key.txt");
    const ARTIFACT: &[u8] = include_bytes!("../../../tests/fixtures/updater/artifact-v0.2.0.bin");
    const GOOD_SIG: &str = include_str!("../../../tests/fixtures/updater/artifact-v0.2.0.bin.sig");
    const WRONG_KEY_SIG: &str =
        include_str!("../../../tests/fixtures/updater/artifact-v0.2.0.wrong-key.sig");

    #[test]
    fn base64_round_trips_known_vectors_and_rejects_garbage() {
        assert_eq!(base64_decode("").unwrap(), b"");
        assert_eq!(base64_decode("Zg==").unwrap(), b"f");
        assert_eq!(base64_decode("Zm8=").unwrap(), b"fo");
        assert_eq!(base64_decode("Zm9v").unwrap(), b"foo");
        assert_eq!(base64_decode("Zm9v\nYmFy").unwrap(), b"foobar");
        assert_eq!(
            base64_decode("Zm9vYg").unwrap(),
            b"foob",
            "unpadded input is accepted"
        );
        assert!(base64_decode("Zm9v!").is_none());
        assert!(base64_decode("Zg==Zg").is_none(), "data after padding");
        assert!(base64_decode("Z===").is_none());
    }

    #[test]
    fn the_production_public_key_decodes_and_has_production_s_key_id() {
        let text = String::from_utf8(base64_decode(PRODUCTION_PUBLIC_KEY).unwrap()).unwrap();
        assert!(
            text.contains("minisign public key: BD4FF4F233057F12"),
            "{text}"
        );
        assert!(TrustRoot::production().is_ok());
    }

    #[test]
    fn a_tauri_signed_artifact_verifies_with_its_key() {
        let root = TrustRoot::from_config_public_key(TEST_KEY).unwrap();
        root.verify(ARTIFACT, GOOD_SIG)
            .expect("signature produced by `tauri signer sign` must verify");
    }

    #[test]
    fn a_corrupted_artifact_is_rejected() {
        let root = TrustRoot::from_config_public_key(TEST_KEY).unwrap();
        for index in [0, 1, ARTIFACT.len() / 2, ARTIFACT.len() - 1] {
            let mut tampered = ARTIFACT.to_vec();
            tampered[index] ^= 0x01;
            assert!(
                matches!(
                    root.verify(&tampered, GOOD_SIG),
                    Err(UpdateError::BadSignature(_))
                ),
                "flip at {index}"
            );
        }
        let mut truncated = ARTIFACT.to_vec();
        truncated.pop();
        assert!(root.verify(&truncated, GOOD_SIG).is_err());
        let mut extended = ARTIFACT.to_vec();
        extended.push(0);
        assert!(root.verify(&extended, GOOD_SIG).is_err());
    }

    #[test]
    fn a_signature_from_a_different_key_is_rejected() {
        let root = TrustRoot::from_config_public_key(TEST_KEY).unwrap();
        assert!(matches!(
            root.verify(ARTIFACT, WRONG_KEY_SIG),
            Err(UpdateError::BadSignature(_))
        ));
        // ...and the production key rejects the test key's (valid) signature.
        let production = TrustRoot::production().unwrap();
        assert!(
            matches!(
                production.verify(ARTIFACT, GOOD_SIG),
                Err(UpdateError::BadSignature(_))
            ),
            "the native app must not trust anything but the embedded key"
        );
    }

    #[test]
    fn missing_empty_and_malformed_signatures_are_rejected() {
        let root = TrustRoot::from_config_public_key(TEST_KEY).unwrap();
        assert_eq!(
            root.verify(ARTIFACT, ""),
            Err(UpdateError::MissingSignature)
        );
        assert_eq!(
            root.verify(ARTIFACT, "   \n"),
            Err(UpdateError::MissingSignature)
        );
        for bad in [
            "not base64 !!!",
            "Zm9v",
            "dW50cnVzdGVkIGNvbW1lbnQ6IHg=",
            GOOD_SIG.trim_end_matches('='),
        ] {
            let outcome = root.verify(ARTIFACT, bad);
            // A merely un-padded copy of the real signature still decodes (padding is optional) and
            // verifies; everything else must fail.
            if bad == GOOD_SIG.trim_end_matches('=') {
                assert!(outcome.is_ok());
            } else {
                assert!(
                    matches!(outcome, Err(UpdateError::BadSignature(_))),
                    "{bad:?}"
                );
            }
        }
    }

    #[test]
    fn a_garbage_trust_root_is_reported_as_a_build_problem() {
        assert!(matches!(
            TrustRoot::from_config_public_key("%%%"),
            Err(UpdateError::BadTrustRoot(_))
        ));
        assert!(matches!(
            TrustRoot::from_config_public_key("Zm9v"),
            Err(UpdateError::BadTrustRoot(_))
        ));
    }
}
