//! `multipart/form-data` bodies (Stage 22a): what the browser builds for production's
//! `FormData` uploads (`/skribbl/submit`: `userId`, `deviceSecret`, `date`, `image`). Built by
//! hand because the one upload in 22a does not justify ureq's `multipart` feature (which pulls
//! `mime_guess`).

/// A form under construction. The boundary is 32 random hex characters from the OS CSPRNG, and
/// is checked against every part so it can never occur inside the payload.
pub struct Multipart {
    boundary: String,
    parts: Vec<(String, Option<(String, String)>, Vec<u8>)>,
}

impl Multipart {
    pub fn new() -> Self {
        let mut bytes = [0u8; 16];
        if getrandom::fill(&mut bytes).is_err() {
            // never expected; a time-derived boundary is still checked against the payload below
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            bytes.copy_from_slice(&t.to_le_bytes());
        }
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Self {
            boundary: format!("----StudyTrackerNative{hex}"),
            parts: Vec::new(),
        }
    }

    pub fn text(mut self, name: &str, value: &str) -> Self {
        self.parts
            .push((name.into(), None, value.as_bytes().to_vec()));
        self
    }

    pub fn file(mut self, name: &str, file_name: &str, content_type: &str, data: Vec<u8>) -> Self {
        self.parts.push((
            name.into(),
            Some((file_name.into(), content_type.into())),
            data,
        ));
        self
    }

    /// `(content-type header, body)`; `None` if the boundary occurs in a part (then the caller
    /// simply builds a new form - a new random boundary).
    pub fn finish(self) -> Option<(String, Vec<u8>)> {
        let marker = format!("--{}", self.boundary);
        if self
            .parts
            .iter()
            .any(|(_, _, data)| data.windows(marker.len()).any(|w| w == marker.as_bytes()))
        {
            return None;
        }
        let mut body = Vec::new();
        for (name, file, data) in &self.parts {
            body.extend_from_slice(marker.as_bytes());
            body.extend_from_slice(b"\r\n");
            match file {
                Some((file_name, content_type)) => body.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\nContent-Type: {content_type}\r\n\r\n"
                    )
                    .as_bytes(),
                ),
                None => body.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
                ),
            }
            body.extend_from_slice(data);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("{marker}--\r\n").as_bytes());
        Some((
            format!("multipart/form-data; boundary={}", self.boundary),
            body,
        ))
    }
}

impl Default for Multipart {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_are_well_formed_and_boundaries_unique() {
        let (ct, body) = Multipart::new()
            .text("userId", "u1")
            .text("date", "2026-10-04")
            .file(
                "image",
                "drawing.png",
                "image/png",
                vec![0x89, b'P', b'N', b'G', 0, 13, 10],
            )
            .finish()
            .unwrap();
        let boundary = ct.strip_prefix("multipart/form-data; boundary=").unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"userId\"\r\n\r\nu1\r\n"
        )));
        assert!(text.contains(
            "name=\"image\"; filename=\"drawing.png\"\r\nContent-Type: image/png\r\n\r\n"
        ));
        assert!(text.ends_with(&format!("--{boundary}--\r\n")));
        let (other, _) = Multipart::new().finish().unwrap();
        assert_ne!(ct, other, "random boundaries");
    }

    #[test]
    fn a_payload_containing_the_boundary_is_refused() {
        let form = Multipart::new();
        let marker = format!("--{}", form.boundary);
        assert!(form
            .file("image", "x.png", "image/png", marker.into_bytes())
            .finish()
            .is_none());
    }
}
