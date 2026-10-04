//! `multipart/form-data` parsing for `/skribbl/submit` (the Worker's `request.formData()`).

pub struct Part {
    pub name: String,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn param(header: &str, key: &str) -> Option<String> {
    let pat = format!("{key}=\"");
    let start = header.find(&pat)? + pat.len();
    let end = header[start..].find('"')? + start;
    Some(header[start..end].to_string())
}

pub fn parse(content_type: &str, body: &[u8]) -> Vec<Part> {
    let Some(boundary) = content_type
        .split("boundary=")
        .nth(1)
        .map(|b| b.trim_matches('"').to_string())
    else {
        return Vec::new();
    };
    let marker = format!("--{boundary}");
    let mut parts = Vec::new();
    let mut pos = match find(body, marker.as_bytes(), 0) {
        Some(p) => p + marker.len(),
        None => return parts,
    };
    loop {
        if body.get(pos..pos + 2) == Some(b"--") {
            break;
        }
        pos += 2; // CRLF after the marker
        let Some(head_end) = find(body, b"\r\n\r\n", pos) else {
            break;
        };
        let head = String::from_utf8_lossy(&body[pos..head_end]).to_string();
        let data_start = head_end + 4;
        let Some(next) = find(body, marker.as_bytes(), data_start) else {
            break;
        };
        let data_end = next.saturating_sub(2);
        let mut name = None;
        let mut file_name = None;
        let mut ctype = None;
        for line in head.split("\r\n") {
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("content-disposition:") {
                name = param(line, "name");
                file_name = param(line, "filename");
            } else if lower.starts_with("content-type:") {
                ctype = Some(line["content-type:".len()..].trim().to_string());
            }
        }
        if let Some(name) = name {
            parts.push(Part {
                name,
                file_name,
                content_type: ctype,
                data: body[data_start..data_end.max(data_start)].to_vec(),
            });
        }
        pos = next + marker.len();
    }
    parts
}
