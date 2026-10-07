use std::io::{stdout, Write};

pub fn copy_to_clipboard(text: &str) -> bool {
    let mut copied = false;

    // 1. Native OS clipboard via arboard
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        if clipboard.set_text(text.to_string()).is_ok() {
            copied = true;
        }
    }

    // 2. OSC 52 escape sequence (supported by modern terminal emulators and across SSH/tmux)
    let base64_text = base64_encode(text.as_bytes());
    let osc52 = format!("\x1b]52;c;{}\x07", base64_text);
    let mut out = stdout();
    if out.write_all(osc52.as_bytes()).is_ok() {
        let _ = out.flush();
        copied = true;
    }

    copied
}

pub fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        result.push(CHARSET[(b0 >> 2) as usize] as char);
        result.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARSET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARSET[(b2 & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

pub fn clean_copied_text(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(|line| line.trim_end()).collect();
    // Drop trailing empty lines from the selection
    let mut end = lines.len();
    while end > 0 && lines[end - 1].is_empty() {
        end -= 1;
    }
    lines[..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"hello world"), "aGVsbG8gd29ybGQ=");
    }

    #[test]
    fn test_clean_copied_text() {
        let raw = "line 1   \nline 2      \n\n   ";
        let cleaned = clean_copied_text(raw);
        assert_eq!(cleaned, "line 1\nline 2");
    }
}
