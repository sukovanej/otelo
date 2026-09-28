//! Message templates: a log body with its variable parts replaced, so lines
//! that differ only in an id or a number group together.

/// Replaces the variable parts of `body` with placeholders.
///
/// A quoted string becomes `<str>`, a UUID `<uuid>`, a hex ID `<hex>`, and a
/// number `<num>`. A number keeps a unit after it (`12ms` is `<num>ms`), and a
/// digit inside a word (`http2`, `user7`) stays.
#[must_use]
pub fn template(body: &str) -> String {
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

/// The index of the quote that closes the one at `start`, skipping a quote
/// after a backslash. A quote that is not closed on the line is text.
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

/// Whether `bytes` starts with a UUID in its 8-4-4-4-12 form, not followed by
/// more of a word.
fn is_uuid(bytes: &[u8]) -> bool {
    if bytes.len() < 36 || bytes.get(36).is_some_and(|&b| is_word(b)) {
        return false;
    }
    bytes[..36].iter().enumerate().all(|(i, &b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

/// A word of 8 or more hex digits with a digit and a letter among them, or a
/// `0x` number.
fn is_hex_id(word: &[u8]) -> bool {
    if let Some(rest) = word.strip_prefix(b"0x") {
        return !rest.is_empty() && rest.iter().all(u8::is_ascii_hexdigit);
    }
    word.len() >= 8
        && word.iter().all(u8::is_ascii_hexdigit)
        && word.iter().any(u8::is_ascii_digit)
        && word.iter().any(u8::is_ascii_alphabetic)
}

/// The end of the digits at `start`, with a decimal part when there is one.
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

/// The length of the UTF-8 sequence that starts with `first`.
const fn utf8_len(first: u8) -> usize {
    match first.leading_ones() {
        2 => 2,
        3 => 3,
        4 => 4,
        _ => 1,
    }
}
