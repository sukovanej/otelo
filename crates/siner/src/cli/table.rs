//! Tables for a terminal, and the formats of the values in them.

use std::fmt::Write as _;
use std::io::{self, Write};

use jiff::Timestamp;
use jiff::tz::TimeZone;
use siner_telemetry::{AttributeValue, Attributes};

/// Rows under a header, each column as wide as its widest cell. A row can
/// have lines under it that the columns do not align.
pub struct Table {
    header: Vec<String>,
    rows: Vec<(Vec<String>, Vec<String>)>,
}

impl Table {
    #[must_use]
    pub fn new(header: &[&str]) -> Self {
        Self {
            header: header.iter().map(|&h| h.to_owned()).collect(),
            rows: Vec::new(),
        }
    }

    pub fn row(&mut self, cells: impl IntoIterator<Item = String>) {
        let cells = cells.into_iter().map(|cell| one_line(&cell)).collect();
        self.rows.push((cells, Vec::new()));
    }

    /// Adds a line under the last row.
    pub fn under(&mut self, line: &str) {
        if let Some((_, lines)) = self.rows.last_mut() {
            lines.push(one_line(line));
        }
    }

    /// Prints the table on stdout.
    ///
    /// # Errors
    ///
    /// When stdout cannot be written.
    pub fn print(&self) -> io::Result<()> {
        let mut out = io::stdout().lock();
        let mut widths: Vec<usize> = self.header.iter().map(|h| h.chars().count()).collect();
        for (cells, _) in &self.rows {
            for (width, cell) in widths.iter_mut().zip(cells) {
                *width = (*width).max(cell.chars().count());
            }
        }
        let line = |out: &mut io::StdoutLock, cells: &[String]| -> io::Result<()> {
            let mut text = String::new();
            for (i, cell) in cells.iter().enumerate() {
                if i + 1 == cells.len() {
                    text.push_str(cell);
                } else {
                    let _ = write!(text, "{cell:<0$}  ", widths[i]);
                }
            }
            writeln!(out, "{}", text.trim_end())
        };
        line(&mut out, &self.header)?;
        for (cells, lines) in &self.rows {
            line(&mut out, cells)?;
            for text in lines {
                writeln!(out, "    {text}")?;
            }
        }
        Ok(())
    }
}

/// `text` on one line, so a cell never breaks a row.
fn one_line(text: &str) -> String {
    text.trim_end()
        .replace('\n', " ↵ ")
        .replace(['\r', '\t'], " ")
}

/// A time in UTC to the millisecond.
#[must_use]
pub fn time(ts: Timestamp) -> String {
    ts.to_zoned(TimeZone::UTC)
        .strftime("%Y-%m-%d %H:%M:%S%.3f")
        .to_string()
}

/// A duration in the unit that suits it: `820µs`, `35ms`, `1.25s`, `3m05s`,
/// `2h30m`.
#[must_use]
pub fn duration(nanos: i64) -> String {
    #[expect(clippy::cast_precision_loss, reason = "a display rounds anyway")]
    let n = nanos as f64;
    match nanos.unsigned_abs() {
        0 => "0s".into(),
        1..1_000 => format!("{nanos}ns"),
        1_000..1_000_000 => format!("{}µs", round(n / 1e3)),
        1_000_000..1_000_000_000 => format!("{}ms", round(n / 1e6)),
        1_000_000_000..60_000_000_000 => format!("{}s", round(n / 1e9)),
        60_000_000_000..3_600_000_000_000 => {
            let seconds = nanos / 1_000_000_000;
            match seconds % 60 {
                0 => format!("{}m", seconds / 60),
                rest => format!("{}m{rest:02}s", seconds / 60),
            }
        }
        _ => {
            let minutes = nanos / 60_000_000_000;
            match minutes % 60 {
                0 => format!("{}h", minutes / 60),
                rest => format!("{}h{rest:02}m", minutes / 60),
            }
        }
    }
}

/// A number with at most three decimals, and none when it is whole.
#[must_use]
pub fn number(value: f64) -> String {
    round(value).to_string()
}

fn round(value: f64) -> f64 {
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// JSON labels as `name=value` pairs.
#[must_use]
pub fn labels(labels: &Attributes) -> String {
    labels
        .iter()
        .map(|(name, value)| match value {
            AttributeValue::String(text) => format!("{name}={text}"),
            other => format!("{name}={other}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}
