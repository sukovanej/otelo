use std::num::NonZeroU8;

use otelo_indexed_storage::query::RankOrder;

use otelo_state::{
    ChartKind, DashboardDefinition, DashboardId, DashboardSummary, GroupedQuery, SpanMeasure,
    StateFile, Widget, WidgetDisplay, WidgetLayout, WidgetQuery,
};

fn requests_dashboard(name: &str) -> DashboardDefinition {
    DashboardDefinition::new(
        name,
        "The requests of every service".into(),
        vec![
            Widget {
                title: "Requests".into(),
                layout: WidgetLayout::new(0, 0, 6, 6).unwrap(),
                display: WidgetDisplay::Timeseries {
                    chart: ChartKind::Bar,
                    queries: vec![GroupedQuery {
                        query: WidgetQuery::Spans {
                            filter: "kind = server".into(),
                            measure: SpanMeasure::Count,
                        },
                        by: vec!["service".into()],
                    }],
                },
            },
            Widget {
                title: "Errors".into(),
                layout: WidgetLayout::new(6, 0, 3, 4).unwrap(),
                display: WidgetDisplay::Toplist {
                    query: GroupedQuery {
                        query: WidgetQuery::Logs {
                            filter: "level >= error".into(),
                        },
                        by: vec!["service".into()],
                    },
                    limit: NonZeroU8::new(5).unwrap(),
                    order: RankOrder::Lowest,
                },
            },
        ],
    )
    .unwrap()
}

#[test]
fn a_dashboard_outlives_the_state_file_until_it_is_deleted() {
    let directory = tempfile::tempdir().unwrap();
    let state = StateFile::open(directory.path()).unwrap();
    assert_eq!(
        state.list_dashboards().unwrap().dashboards,
        Vec::<DashboardSummary>::new()
    );
    let created = state.create_dashboard(&requests_dashboard("API")).unwrap();
    let other = state
        .create_dashboard(&requests_dashboard("Workers"))
        .unwrap();
    drop(state);

    let state = StateFile::open(directory.path()).unwrap();
    assert_eq!(
        state.find_dashboard(created.id).unwrap(),
        Some(created.clone())
    );
    let replaced = state
        .replace_dashboard(created.id, &requests_dashboard("API, again"))
        .unwrap()
        .unwrap();
    assert_eq!(replaced.definition.name(), "API, again");
    assert_eq!(replaced.created_at, created.created_at);
    assert!(replaced.updated_at >= created.updated_at);

    let names: Vec<_> = state
        .list_dashboards()
        .unwrap()
        .dashboards
        .into_iter()
        .map(|summary| (summary.name, summary.widget_count))
        .collect();
    assert_eq!(
        names,
        [("API, again".to_owned(), 2), ("Workers".to_owned(), 2)]
    );

    assert!(state.delete_dashboard(other.id).unwrap());
    assert!(!state.delete_dashboard(other.id).unwrap());
    assert_eq!(state.find_dashboard(other.id).unwrap(), None);
    assert_eq!(
        state
            .replace_dashboard("99".parse().unwrap(), &requests_dashboard("Nothing"))
            .unwrap(),
        None
    );
}

#[test]
fn a_dashboard_definition_reads_back_from_its_json() {
    let definition = requests_dashboard("API");
    let json = serde_json::to_value(&definition).unwrap();
    assert_eq!(
        json["widgets"][0]["display"],
        serde_json::json!({
            "kind": "timeseries",
            "chart": "bar",
            "queries": [{
                "query": {"signal": "spans", "filter": "kind = server", "measure": "count"},
                "by": ["service"],
            }],
        })
    );
    assert_eq!(
        json["widgets"][1]["layout"],
        serde_json::json!({"column": 6, "row": 0, "width": 3, "height": 4})
    );
    assert_eq!(
        serde_json::from_value::<DashboardDefinition>(json).unwrap(),
        definition
    );
}

#[test]
fn a_widget_takes_an_area_inside_the_grid() {
    assert!(WidgetLayout::new(0, 0, 0, 4).is_err());
    assert!(WidgetLayout::new(0, 0, 13, 4).is_err());
    assert!(WidgetLayout::new(9, 0, 4, 4).is_err());
    assert!(WidgetLayout::new(0, 0, 4, 1).is_err());
    assert!(WidgetLayout::new(0, 10_000, 4, 4).is_err());
    let widget = serde_json::json!({
        "title": "",
        "layout": {"column": 8, "row": 0, "width": 6, "height": 4},
        "display": {"kind": "note", "text": ""},
    });
    assert!(serde_json::from_value::<Widget>(widget).is_err());

    let left = WidgetLayout::new(0, 0, 6, 4).unwrap();
    assert!(!left.overlaps(&WidgetLayout::new(6, 0, 6, 4).unwrap()));
    assert!(!left.overlaps(&WidgetLayout::new(0, 4, 6, 4).unwrap()));
    assert!(left.overlaps(&WidgetLayout::new(5, 3, 2, 2).unwrap()));
}

fn read_dashboard_json(widgets: &serde_json::Value) -> Result<DashboardDefinition, String> {
    serde_json::from_value(serde_json::json!({
        "name": "API",
        "description": "",
        "widgets": widgets,
    }))
    .map_err(|error| error.to_string())
}

fn spans_widget(filter: &str, by: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "title": "Requests",
        "layout": {"column": 0, "row": 0, "width": 6, "height": 6},
        "display": {
            "kind": "timeseries",
            "chart": "line",
            "queries": [{
                "query": {"signal": "spans", "filter": filter, "measure": "p95"},
                "by": by,
            }],
        },
    })
}

#[test]
fn a_dashboard_saves_only_queries_its_signals_can_run() {
    let widgets = serde_json::json!([spans_widget("kind = server", &["service", "http.route"])]);
    assert!(read_dashboard_json(&widgets).is_ok());
    let error = read_dashboard_json(&serde_json::json!([spans_widget("kind = = server", &[])]))
        .unwrap_err();
    assert!(error.starts_with("widget 1 (Requests): "), "{error}");
    assert!(read_dashboard_json(&serde_json::json!([spans_widget("", &["duration"])])).is_err());
    let unnamed_metric = serde_json::json!([{
        "title": "",
        "layout": {"column": 0, "row": 0, "width": 12, "height": 4},
        "display": {"kind": "value", "query": {
            "signal": "metrics", "name": " ", "filter": "", "aggregation": "avg",
        }},
    }]);
    assert!(read_dashboard_json(&unnamed_metric).is_err());
    let no_queries = serde_json::json!([{
        "title": "",
        "layout": {"column": 0, "row": 0, "width": 12, "height": 4},
        "display": {"kind": "timeseries", "chart": "area", "queries": []},
    }]);
    assert!(read_dashboard_json(&no_queries).is_err());
}

#[test]
fn a_dashboard_needs_a_name_and_keeps_its_texts_short() {
    assert!(DashboardDefinition::new("  ", String::new(), Vec::new()).is_err());
    let trimmed = DashboardDefinition::new(" API ", String::new(), Vec::new()).unwrap();
    assert_eq!(trimmed.name(), "API");
    assert!(DashboardDefinition::new("API", "x".repeat(1001), Vec::new()).is_err());
    let long_note = serde_json::json!([{
        "title": "",
        "layout": {"column": 0, "row": 0, "width": 6, "height": 4},
        "display": {"kind": "note", "text": "x".repeat(10_001)},
    }]);
    assert!(read_dashboard_json(&long_note).is_err());
}

#[test]
fn widgets_of_a_dashboard_never_overlap() {
    let widget_at = |column: u8, row: u16| {
        serde_json::json!({
            "title": "",
            "layout": {"column": column, "row": row, "width": 6, "height": 4},
            "display": {"kind": "note", "text": ""},
        })
    };
    assert!(read_dashboard_json(&serde_json::json!([widget_at(0, 0), widget_at(6, 0)])).is_ok());
    let overlapping = serde_json::json!([widget_at(0, 0), widget_at(6, 0), widget_at(3, 2)]);
    let error = read_dashboard_json(&overlapping).unwrap_err();
    assert!(
        error.starts_with("widgets 1 and 3 overlap on the grid"),
        "{error}"
    );
}

#[test]
fn a_dashboard_id_is_a_whole_number() {
    assert_eq!("12".parse::<DashboardId>().unwrap().to_string(), "12");
    assert!("twelve".parse::<DashboardId>().is_err());
}
