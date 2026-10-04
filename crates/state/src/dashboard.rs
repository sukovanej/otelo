use std::fmt;
use std::num::NonZeroU8;
use std::str::FromStr;

use anyhow::Context;
use jiff::Timestamp;
use otelo_indexed_storage::query::{GroupingField, LogGroupingField, RankOrder, SpanGroupingField};
use otelo_query::Signal;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::StateFile;

const GRID_COLUMNS: u8 = 12;

const MAX_WIDGET_START_ROW: u16 = 9999;

const MIN_WIDGET_ROWS: u8 = 2;

const MAX_WIDGET_ROWS: u8 = 16;

const MAX_NAME_CHARS: usize = 200;

const MAX_DESCRIPTION_CHARS: usize = 1000;

const MAX_TITLE_CHARS: usize = 200;

const MAX_NOTE_CHARS: usize = 10_000;

const MAX_WIDGETS_PER_DASHBOARD: usize = 100;

const MAX_QUERIES_PER_TIMESERIES: usize = 8;

/// The ID of a saved dashboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct DashboardId(i64);

impl FromStr for DashboardId {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.parse()
            .map(Self)
            .map_err(|_| format!("a dashboard ID is a whole number, not {text:?}"))
    }
}

impl fmt::Display for DashboardId {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A saved dashboard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Dashboard {
    pub id: DashboardId,
    pub definition: DashboardDefinition,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

/// What a dashboard shows: its name, its description, and its widgets, no two
/// of which overlap on the grid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "DashboardDefinitionFields")]
pub struct DashboardDefinition {
    /// The name, without the spaces around it; 1 to 200 characters.
    name: String,
    /// Up to 1000 characters.
    description: String,
    /// Up to 100 widgets.
    widgets: Vec<Widget>,
}

#[derive(Deserialize)]
struct DashboardDefinitionFields {
    name: String,
    description: String,
    widgets: Vec<Widget>,
}

impl DashboardDefinition {
    pub fn new(name: &str, description: String, widgets: Vec<Widget>) -> Result<Self, String> {
        Self::try_from(DashboardDefinitionFields {
            name: name.to_owned(),
            description,
            widgets,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn widgets(&self) -> &[Widget] {
        &self.widgets
    }
}

impl TryFrom<DashboardDefinitionFields> for DashboardDefinition {
    type Error = String;

    fn try_from(fields: DashboardDefinitionFields) -> Result<Self, Self::Error> {
        let name = fields.name.trim();
        if name.is_empty() {
            return Err("a dashboard needs a name".to_owned());
        }
        check_char_count("the name of a dashboard", name, MAX_NAME_CHARS)?;
        check_char_count(
            "the description of a dashboard",
            &fields.description,
            MAX_DESCRIPTION_CHARS,
        )?;
        if fields.widgets.len() > MAX_WIDGETS_PER_DASHBOARD {
            return Err(format!(
                "a dashboard has {MAX_WIDGETS_PER_DASHBOARD} widgets at most"
            ));
        }
        for (index, widget) in fields.widgets.iter().enumerate() {
            if let Some(other_index) = fields.widgets[..index]
                .iter()
                .position(|other| other.layout.overlaps(&widget.layout))
            {
                return Err(format!(
                    "widgets {} and {} overlap on the grid",
                    other_index + 1,
                    index + 1
                ));
            }
            check_widget(widget).map_err(|error| {
                let title = if widget.title.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", widget.title)
                };
                format!("widget {}{title}: {error}", index + 1)
            })?;
        }
        Ok(Self {
            name: name.to_owned(),
            description: fields.description,
            widgets: fields.widgets,
        })
    }
}

/// The saved dashboards, the most recently changed first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct DashboardList {
    pub dashboards: Vec<DashboardSummary>,
}

/// A saved dashboard without its widgets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DashboardSummary {
    pub id: DashboardId,
    pub name: String,
    pub description: String,
    pub widget_count: u32,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

/// One tile of a dashboard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Widget {
    pub title: String,
    pub layout: WidgetLayout,
    pub display: WidgetDisplay,
}

/// The area a widget takes on the grid of 12 columns of a wide screen.
///
/// A narrower screen has a grid of 6 or 2 columns, and the browser derives the
/// places there from these. A row has the same height on every screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "WidgetLayoutFields")]
pub struct WidgetLayout {
    /// The first column of the widget, from 0 at the left.
    #[schema(minimum = 0, maximum = 11)]
    column: u8,
    /// The first row of the widget, from 0 at the top.
    #[schema(minimum = 0, maximum = 9999)]
    row: u16,
    /// The columns the widget spans.
    #[schema(minimum = 1, maximum = 12)]
    width: u8,
    /// The rows the widget spans.
    #[schema(minimum = 2, maximum = 16)]
    height: u8,
}

#[derive(Deserialize)]
struct WidgetLayoutFields {
    column: u8,
    row: u16,
    width: u8,
    height: u8,
}

impl WidgetLayout {
    pub fn new(column: u8, row: u16, width: u8, height: u8) -> Result<Self, String> {
        Self::try_from(WidgetLayoutFields {
            column,
            row,
            width,
            height,
        })
    }

    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        let columns = u16::from(self.column)..u16::from(self.column) + u16::from(self.width);
        let other_columns =
            u16::from(other.column)..u16::from(other.column) + u16::from(other.width);
        let rows = self.row..self.row + u16::from(self.height);
        let other_rows = other.row..other.row + u16::from(other.height);
        columns.start < other_columns.end
            && other_columns.start < columns.end
            && rows.start < other_rows.end
            && other_rows.start < rows.end
    }
}

impl TryFrom<WidgetLayoutFields> for WidgetLayout {
    type Error = String;

    fn try_from(fields: WidgetLayoutFields) -> Result<Self, Self::Error> {
        if !(1..=GRID_COLUMNS).contains(&fields.width) {
            return Err(format!(
                "a widget spans 1 to {GRID_COLUMNS} columns, not {}",
                fields.width
            ));
        }
        if u16::from(fields.column) + u16::from(fields.width) > u16::from(GRID_COLUMNS) {
            return Err(format!(
                "a widget of {} columns starts at column {} at most, not {}",
                fields.width,
                GRID_COLUMNS - fields.width,
                fields.column
            ));
        }
        if !(MIN_WIDGET_ROWS..=MAX_WIDGET_ROWS).contains(&fields.height) {
            return Err(format!(
                "a widget spans {MIN_WIDGET_ROWS} to {MAX_WIDGET_ROWS} rows, not {}",
                fields.height
            ));
        }
        if fields.row > MAX_WIDGET_START_ROW {
            return Err(format!(
                "a widget starts at row {MAX_WIDGET_START_ROW} at most, not {}",
                fields.row
            ));
        }
        Ok(Self {
            column: fields.column,
            row: fields.row,
            width: fields.width,
            height: fields.height,
        })
    }
}

/// What a widget draws.
///
/// `timeseries` charts each query over the range, a line per group. `value`
/// is one number over the range, with its trend. `toplist` ranks the groups
/// of one query. `note` is text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum WidgetDisplay {
    Timeseries {
        chart: ChartKind,
        queries: Vec<GroupedQuery>,
    },
    Value {
        query: WidgetQuery,
    },
    Toplist {
        query: GroupedQuery,
        /// The most groups to rank.
        #[schema(value_type = u8, minimum = 1)]
        limit: NonZeroU8,
        /// Whether the groups with the `highest` number come first, or the
        /// `lowest`. `highest` when missing.
        #[serde(default)]
        order: RankOrder,
    },
    Note {
        text: String,
    },
}

/// How a time series draws its values: lines, lines over a filled area, or
/// stacked bars.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChartKind {
    Line,
    Area,
    Bar,
}

/// A query and the names to group what it reads by.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GroupedQuery {
    pub query: WidgetQuery,
    /// The names to group by, as a query writes them, such as `service` or
    /// `http.route`. Everything is one group when empty.
    pub by: Vec<String>,
}

/// The telemetry a widget reads, and the number it takes of it.
///
/// `spans` and `logs` read the records that `filter` keeps, in the query
/// language of their signal. `metrics` reads the series of the metric
/// `name` that `filter` keeps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "signal", rename_all = "lowercase")]
pub enum WidgetQuery {
    Spans {
        filter: String,
        measure: SpanMeasure,
    },
    Logs {
        filter: String,
    },
    Metrics {
        name: String,
        filter: String,
        aggregation: MetricAggregation,
    },
}

/// The number a widget takes of spans: their count, their count per second,
/// their failures, the share of them that failed, or a percentile of their
/// durations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpanMeasure {
    Count,
    Rate,
    Errors,
    ErrorRate,
    P50,
    P95,
    P99,
}

/// The number a widget takes of the points of a metric in a step. `avg`,
/// `min`, `max`, and `last` fit a gauge and an updown, `rate` a counter, and
/// the percentiles a histogram.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MetricAggregation {
    Avg,
    Min,
    Max,
    Last,
    Rate,
    P50,
    P90,
    P99,
}

const DASHBOARD_SUMMARY_COLUMNS: &str =
    "id, name, description, json_array_length(widgets), created_at, updated_at";

impl StateFile {
    pub fn list_dashboards(&self) -> anyhow::Result<DashboardList> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(&format!(
            "SELECT {DASHBOARD_SUMMARY_COLUMNS}
             FROM dashboards
             ORDER BY updated_at DESC, id DESC"
        ))?;
        let dashboards = statement
            .query_map([], |row| {
                Ok(DashboardSummary {
                    id: DashboardId(row.get(0)?),
                    name: row.get(1)?,
                    description: row.get(2)?,
                    widget_count: row.get(3)?,
                    created_at: timestamp_from_nanos(row.get(4)?),
                    updated_at: timestamp_from_nanos(row.get(5)?),
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(DashboardList { dashboards })
    }

    pub fn find_dashboard(&self, id: DashboardId) -> anyhow::Result<Option<Dashboard>> {
        let row = self
            .open_connection()?
            .query_row(
                "SELECT name, description, widgets, created_at, updated_at
                 FROM dashboards
                 WHERE id = ?1",
                [id.0],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(name, description, widgets, created_at, updated_at)| {
            Ok(Dashboard {
                id,
                definition: DashboardDefinition {
                    name,
                    description,
                    widgets: serde_json::from_str(&widgets)
                        .with_context(|| format!("read the widgets of dashboard {}", id.0))?,
                },
                created_at: timestamp_from_nanos(created_at),
                updated_at: timestamp_from_nanos(updated_at),
            })
        })
        .transpose()
    }

    pub fn create_dashboard(&self, definition: &DashboardDefinition) -> anyhow::Result<Dashboard> {
        let now = unix_nanos_now()?;
        let connection = self.open_connection()?;
        connection.execute(
            "INSERT INTO dashboards (name, description, widgets, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            (
                &definition.name,
                &definition.description,
                serde_json::to_string(&definition.widgets)?,
                now,
            ),
        )?;
        Ok(Dashboard {
            id: DashboardId(connection.last_insert_rowid()),
            definition: definition.clone(),
            created_at: timestamp_from_nanos(now),
            updated_at: timestamp_from_nanos(now),
        })
    }

    pub fn replace_dashboard(
        &self,
        id: DashboardId,
        definition: &DashboardDefinition,
    ) -> anyhow::Result<Option<Dashboard>> {
        let replaced = self.open_connection()?.execute(
            "UPDATE dashboards
             SET name = ?2, description = ?3, widgets = ?4, updated_at = ?5
             WHERE id = ?1",
            (
                id.0,
                &definition.name,
                &definition.description,
                serde_json::to_string(&definition.widgets)?,
                unix_nanos_now()?,
            ),
        )?;
        if replaced == 0 {
            return Ok(None);
        }
        self.find_dashboard(id)
    }

    pub fn delete_dashboard(&self, id: DashboardId) -> anyhow::Result<bool> {
        let deleted = self
            .open_connection()?
            .execute("DELETE FROM dashboards WHERE id = ?1", [id.0])?;
        Ok(deleted > 0)
    }
}

fn unix_nanos_now() -> anyhow::Result<i64> {
    Ok(i64::try_from(Timestamp::now().as_nanosecond())?)
}

fn timestamp_from_nanos(unix_nanos: i64) -> Timestamp {
    Timestamp::from_nanosecond(i128::from(unix_nanos))
        .expect("an i64 of nanoseconds is a valid timestamp")
}

fn check_widget(widget: &Widget) -> Result<(), String> {
    check_char_count("the title of a widget", &widget.title, MAX_TITLE_CHARS)?;
    match &widget.display {
        WidgetDisplay::Timeseries { queries, .. } => {
            if queries.is_empty() || queries.len() > MAX_QUERIES_PER_TIMESERIES {
                return Err(format!(
                    "a time series has 1 to {MAX_QUERIES_PER_TIMESERIES} queries"
                ));
            }
            queries.iter().try_for_each(check_grouped_query)
        }
        WidgetDisplay::Value { query } => check_query(query),
        WidgetDisplay::Toplist { query, .. } => check_grouped_query(query),
        WidgetDisplay::Note { text } => check_char_count("a note", text, MAX_NOTE_CHARS),
    }
}

fn check_grouped_query(grouped: &GroupedQuery) -> Result<(), String> {
    check_query(&grouped.query)?;
    let by = grouped.by.iter().map(String::as_str);
    match grouped.query {
        WidgetQuery::Spans { .. } => check_grouping_fields::<SpanGroupingField>(by),
        WidgetQuery::Logs { .. } => check_grouping_fields::<LogGroupingField>(by),
        WidgetQuery::Metrics { .. } => check_grouping_fields::<GroupingField>(by),
    }
}

fn check_query(query: &WidgetQuery) -> Result<(), String> {
    let (filter, signal) = match query {
        WidgetQuery::Spans { filter, .. } => (filter, Signal::Spans),
        WidgetQuery::Logs { filter } => (filter, Signal::Logs),
        WidgetQuery::Metrics { name, filter, .. } => {
            if name.trim().is_empty() {
                return Err("a metric query needs the name of a metric".to_owned());
            }
            (filter, Signal::Metrics)
        }
    };
    otelo_query::parse_query(filter, signal)
        .map(drop)
        .map_err(|error| error.to_string())
}

fn check_grouping_fields<'a, Field: FromStr<Err = String>>(
    mut names: impl Iterator<Item = &'a str>,
) -> Result<(), String> {
    names.try_for_each(|name| name.parse::<Field>().map(drop))
}

fn check_char_count(what: &str, text: &str, max_chars: usize) -> Result<(), String> {
    if text.chars().count() > max_chars {
        return Err(format!("{what} has {max_chars} characters at most"));
    }
    Ok(())
}
