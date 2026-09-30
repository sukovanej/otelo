use std::fmt::Write;

use otelo_query::{HighlightKind, highlight_tokens};

const fn mark_of_kind(kind: HighlightKind) -> char {
    match kind {
        HighlightKind::Field => 'f',
        HighlightKind::UndecidedWord => 'w',
        HighlightKind::Operator => 'o',
        HighlightKind::Keyword => 'k',
        HighlightKind::String => 's',
        HighlightKind::Number => 'n',
        HighlightKind::Boolean => 'b',
        HighlightKind::Punctuation => 'p',
        HighlightKind::Invalid => 'x',
    }
}

fn mark_each_char(query: &str) -> String {
    let highlights = highlight_tokens(query);
    query
        .char_indices()
        .map(|(byte_offset, _)| {
            highlights
                .iter()
                .find(|highlight| highlight.byte_range.contains(&byte_offset))
                .map_or(' ', |highlight| mark_of_kind(highlight.kind))
        })
        .collect()
}

// packages/app/tests/highlight.test.ts reads these snapshots: a query after
// "> ", and under it one mark for each of its characters.
#[test]
fn tokens() {
    insta::glob!("highlight/*.txt", |path| {
        let mut out = String::new();
        for query in std::fs::read_to_string(path).unwrap().lines() {
            writeln!(out, "> {query}").unwrap();
            writeln!(out, "{}", format!("  {}", mark_each_char(query)).trim_end()).unwrap();
        }
        insta::assert_snapshot!(out);
    });
}
