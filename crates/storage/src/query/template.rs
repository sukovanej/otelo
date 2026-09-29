#[must_use]
pub fn message_template(body: &str) -> String {
    let bytes = body.as_bytes();
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let at_boundary = i == 0 || !is_word(bytes[i - 1]);
        if at_boundary
            && (c == b'"' || c == b'\'')
            && let Some(end) = closing_quote(bytes, i)
        {
            out.push(char::from(c));
            out.push_str("<str>");
            out.push(char::from(c));
            i = end + 1;
            continue;
        }
        if at_boundary && c.is_ascii_alphanumeric() {
            let end = i + bytes[i..]
                .iter()
                .take_while(|b| b.is_ascii_alphanumeric())
                .count();
            if is_uuid(&bytes[i..]) {
                out.push_str("<uuid>");
                i += 36;
                continue;
            }
            let word = &bytes[i..end];
            if is_hex_id(word) {
                out.push_str("<hex>");
                i = end;
                continue;
            }
            if c.is_ascii_digit() {
                out.push_str("<num>");
                i = number_end(bytes, i);
                continue;
            }
            out.push_str(&body[i..end]);
            i = end;
            continue;
        }
        let len = utf8_len(c);
        out.push_str(&body[i..i + len]);
        i += len;
    }
    out
}

const fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

const fn closing_quote(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'\n' => return None,
            b if b == quote => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn is_uuid(bytes: &[u8]) -> bool {
    if bytes.len() < 36 || bytes.get(36).is_some_and(|&b| is_word(b)) {
        return false;
    }
    bytes[..36].iter().enumerate().all(|(i, &b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

fn is_hex_id(word: &[u8]) -> bool {
    if let Some(rest) = word.strip_prefix(b"0x") {
        return !rest.is_empty() && rest.iter().all(u8::is_ascii_hexdigit);
    }
    word.len() >= 8
        && word.iter().all(u8::is_ascii_hexdigit)
        && word.iter().any(u8::is_ascii_digit)
        && word.iter().any(u8::is_ascii_alphabetic)
}

fn number_end(bytes: &[u8], start: usize) -> usize {
    let digits = |from: usize| {
        from + bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let end = digits(start);
    if bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
        digits(end + 1)
    } else {
        end
    }
}

const fn utf8_len(first: u8) -> usize {
    match first.leading_ones() {
        2 => 2,
        3 => 3,
        4 => 4,
        _ => 1,
    }
}
