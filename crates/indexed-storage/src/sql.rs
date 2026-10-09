// The SQL values of `db.system.name`, and of the `db.system` it replaced, in the OpenTelemetry
// semantic conventions.
const SQL_SYSTEMS: [&str; 50] = [
    "actian.ingres",
    "aws.redshift",
    "clickhouse",
    "cockroachdb",
    "derby",
    "firebirdsql",
    "gcp.spanner",
    "h2database",
    "hive",
    "hsqldb",
    "ibm.db2",
    "ibm.informix",
    "ibm.netezza",
    "instantdb",
    "intersystems.cache",
    "mariadb",
    "microsoft.sql_server",
    "mysql",
    "oracle.db",
    "other_sql",
    "postgresql",
    "sap.hana",
    "sap.maxdb",
    "sqlite",
    "teradata",
    "trino",
    "cache",
    "cloudscape",
    "db2",
    "edb",
    "firebird",
    "firstsql",
    "h2",
    "hanadb",
    "informix",
    "ingres",
    "interbase",
    "intersystems_cache",
    "maxdb",
    "mssql",
    "mssqlcompact",
    "netezza",
    "oracle",
    "pervasive",
    "pointbase",
    "progress",
    "redshift",
    "spanner",
    "sybase",
    "vertica",
];

const PLACEHOLDER: char = '?';

#[must_use]
pub fn is_sql_system(system: &str) -> bool {
    SQL_SYSTEMS.contains(&system)
}

// The OpenTelemetry semantic conventions ask that a query keep no literal, and that a query
// whose values are bound stays as it is. None when the query has no literal.
#[must_use]
pub fn sanitize_sql_query_text(text: &str) -> Option<String> {
    let mut sanitized = String::with_capacity(text.len());
    let mut replaced_literal = false;
    let mut rest = text;
    while let Some(character) = rest.chars().next() {
        let previous = sanitized.chars().next_back();
        let at_word_start = previous.is_none_or(|previous| !is_word_character(previous));
        let consumed = if character == '\'' {
            find_string_literal_end(rest).inspect(|_| {
                sanitized.push(PLACEHOLDER);
                replaced_literal = true;
            })
        } else if matches!(character, 'x' | 'X') && at_word_start && rest[1..].starts_with('\'') {
            find_string_literal_end(&rest[1..]).map(|end| {
                sanitized.push(PLACEHOLDER);
                replaced_literal = true;
                end + 1
            })
        } else if character.is_ascii_digit()
            && at_word_start
            && previous.is_none_or(|previous| !matches!(previous, '?' | '$' | ':' | '@' | '.'))
        {
            sanitized.push(PLACEHOLDER);
            replaced_literal = true;
            Some(find_number_end(rest))
        } else if matches!(character, '"' | '`') {
            let end = rest[1..].find(character).map_or(rest.len(), |end| end + 2);
            sanitized.push_str(&rest[..end]);
            Some(end)
        } else if rest.starts_with("--") {
            let end = rest.find('\n').unwrap_or(rest.len());
            sanitized.push_str(&rest[..end]);
            Some(end)
        } else {
            None
        };
        let consumed = consumed.unwrap_or_else(|| {
            sanitized.push(character);
            character.len_utf8()
        });
        rest = &rest[consumed..];
    }
    replaced_literal.then(|| collapse_repeated_groups(&collapse_placeholder_lists(&sanitized)))
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn find_string_literal_end(text: &str) -> Option<usize> {
    let mut position = 1;
    while let Some(offset) = text[position..].find('\'') {
        position += offset + 1;
        if !text[position..].starts_with('\'') {
            return Some(position);
        }
        position += 1;
    }
    None
}

fn find_number_end(text: &str) -> usize {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return 2 + hex
            .find(|character: char| !character.is_ascii_hexdigit())
            .unwrap_or(hex.len());
    }
    let mut end = 0;
    let bytes = text.as_bytes();
    let take_digits = |end: &mut usize| {
        while bytes.get(*end).is_some_and(u8::is_ascii_digit) {
            *end += 1;
        }
    };
    take_digits(&mut end);
    if bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
        end += 1;
        take_digits(&mut end);
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
        if bytes.get(end + 1 + sign).is_some_and(u8::is_ascii_digit) {
            end += 1 + sign;
            take_digits(&mut end);
        }
    }
    end
}

// A list written with its values has a text for each of its lengths.
fn collapse_placeholder_lists(text: &str) -> String {
    collapse_repeated_items(text, |rest| {
        rest.starts_with(PLACEHOLDER)
            .then_some(1)
            .filter(|&end| !rest[end..].starts_with(is_word_character))
    })
}

fn collapse_repeated_groups(text: &str) -> String {
    collapse_repeated_items(text, |rest| {
        if !rest.starts_with('(') {
            return None;
        }
        let end = rest[1..].find(['(', ')'])? + 1;
        rest[end..].starts_with(')').then_some(end + 1)
    })
}

fn collapse_repeated_items(text: &str, find_item_end: impl Fn(&str) -> Option<usize>) -> String {
    let mut collapsed = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(character) = rest.chars().next() {
        let at_word_start = collapsed
            .chars()
            .next_back()
            .is_none_or(|previous| !is_word_character(previous));
        let Some(item_end) = find_item_end(rest).filter(|_| at_word_start) else {
            collapsed.push(character);
            rest = &rest[character.len_utf8()..];
            continue;
        };
        let item = &rest[..item_end];
        collapsed.push_str(item);
        rest = &rest[item_end..];
        while let Some(after_comma) = rest.trim_start().strip_prefix(',') {
            let next = after_comma.trim_start();
            if find_item_end(next).is_some_and(|next_end| next[..next_end] == *item) {
                rest = &next[item.len()..];
            } else {
                break;
            }
        }
    }
    collapsed
}
