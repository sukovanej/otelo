#[must_use]
pub fn replace_values_in_message(body: &str) -> String {
    let bytes = body.as_bytes();
    let mut template = String::with_capacity(body.len());
    let mut position = 0;
    while position < bytes.len() {
        let byte = bytes[position];
        let at_boundary = position == 0 || !is_word_byte(bytes[position - 1]);
        if at_boundary
            && (byte == b'"' || byte == b'\'')
            && let Some(end) = find_closing_quote(bytes, position)
        {
            template.push(char::from(byte));
            template.push_str("<str>");
            template.push(char::from(byte));
            position = end + 1;
            continue;
        }
        if at_boundary && byte.is_ascii_alphanumeric() {
            let end = position
                + bytes[position..]
                    .iter()
                    .take_while(|byte| byte.is_ascii_alphanumeric())
                    .count();
            if starts_with_uuid(&bytes[position..]) {
                template.push_str("<uuid>");
                position += 36;
                continue;
            }
            let word = &bytes[position..end];
            if is_hex_id(word) {
                template.push_str("<hex>");
                position = end;
                continue;
            }
            if byte.is_ascii_digit() {
                template.push_str("<num>");
                position = find_number_end(bytes, position);
                continue;
            }
            template.push_str(&body[position..end]);
            position = end;
            continue;
        }
        let char_len = utf8_char_len(byte);
        template.push_str(&body[position..position + char_len]);
        position += char_len;
    }
    template
}

const fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

const fn find_closing_quote(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = bytes[start];
    let mut position = start + 1;
    while position < bytes.len() {
        match bytes[position] {
            b'\\' => position += 2,
            b'\n' => return None,
            byte if byte == quote => return Some(position),
            _ => position += 1,
        }
    }
    None
}

fn starts_with_uuid(bytes: &[u8]) -> bool {
    if bytes.len() < 36 || bytes.get(36).is_some_and(|&byte| is_word_byte(byte)) {
        return false;
    }
    bytes[..36]
        .iter()
        .enumerate()
        .all(|(position, &byte)| match position {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
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

fn find_number_end(bytes: &[u8], start: usize) -> usize {
    let end_of_digits = |from: usize| {
        from + bytes[from..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count()
    };
    let end = end_of_digits(start);
    if bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
        end_of_digits(end + 1)
    } else {
        end
    }
}

const fn utf8_char_len(first_byte: u8) -> usize {
    match first_byte.leading_ones() {
        2 => 2,
        3 => 3,
        4 => 4,
        _ => 1,
    }
}
