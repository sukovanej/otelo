use std::fmt::Write as _;
use std::io::{self, Write};

use jiff::Timestamp;
use jiff::tz::TimeZone;
use otelo_storage::{AttributeValue, Attributes};

pub struct Table {
    header: Vec<String>,
    rows: Vec<Row>,
}

struct Row {
    cells: Vec<String>,
    unaligned_lines_under: Vec<String>,
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
        let cells = cells
            .into_iter()
            .map(|cell| flatten_to_one_line(&cell))
            .collect();
        self.rows.push(Row {
            cells,
            unaligned_lines_under: Vec::new(),
        });
    }

    pub fn add_line_under_last_row(&mut self, line: &str) {
        if let Some(row) = self.rows.last_mut() {
            row.unaligned_lines_under.push(flatten_to_one_line(line));
        }
    }

    pub fn print(&self) -> io::Result<()> {
        let mut out = io::stdout().lock();
        let mut widths: Vec<usize> = self.header.iter().map(|h| h.chars().count()).collect();
        for row in &self.rows {
            for (width, cell) in widths.iter_mut().zip(&row.cells) {
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
        for row in &self.rows {
            line(&mut out, &row.cells)?;
            for text in &row.unaligned_lines_under {
                writeln!(out, "    {text}")?;
            }
        }
        Ok(())
    }
}

fn flatten_to_one_line(text: &str) -> String {
    text.trim_end()
        .replace('\n', " ↵ ")
        .replace(['\r', '\t'], " ")
}

#[must_use]
pub fn format_utc_time(ts: Timestamp) -> String {
    ts.to_zoned(TimeZone::UTC)
        .strftime("%Y-%m-%d %H:%M:%S%.3f")
        .to_string()
}

#[must_use]
pub fn format_duration(nanos: i64) -> String {
    #[expect(clippy::cast_precision_loss, reason = "a display rounds anyway")]
    let n = nanos as f64;
    match nanos.unsigned_abs() {
        0 => "0s".into(),
        1..1_000 => format!("{nanos}ns"),
        1_000..1_000_000 => format!("{}µs", round_to_three_decimals(n / 1e3)),
        1_000_000..1_000_000_000 => format!("{}ms", round_to_three_decimals(n / 1e6)),
        1_000_000_000..60_000_000_000 => format!("{}s", round_to_three_decimals(n / 1e9)),
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

#[must_use]
pub fn format_number(value: f64) -> String {
    round_to_three_decimals(value).to_string()
}

fn round_to_three_decimals(value: f64) -> f64 {
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded == 0.0 { 0.0 } else { rounded }
}

#[must_use]
pub fn format_labels(labels: &Attributes) -> String {
    labels
        .iter()
        .map(|(name, value)| match value {
            AttributeValue::String(text) => format!("{name}={text}"),
            other => format!("{name}={other}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}
