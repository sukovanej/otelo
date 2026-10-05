mod catalog;
pub mod client;
mod dashboards;
mod login;
mod logs;
mod metrics;
mod remote;
mod services;
mod span_groups;
pub mod table;
mod traces;

use otelo_indexed_storage::query::{Logs, MAX_PAGE_ROWS, PageCursor, Spans, Traces};
use otelo_query::Signal;
use serde::de::DeserializeOwned;

use client::Client;

pub use catalog::{
    AttributesArgs, CompleteArgs, IndexArgs, change_and_print_indexes, print_attributes,
    print_completions,
};
pub use dashboards::{DashboardArgs, change_and_print_dashboards};
pub use login::{LoginArgs, LogoutArgs, delete_daemon_password, store_daemon_password};
pub use logs::{LogsArgs, print_logs};
pub use metrics::{MetricArgs, MetricsArgs, print_metric_series, print_metrics};
pub use remote::{RemoteArgs, change_and_print_remotes};
pub use services::{ServiceArgs, ServicesArgs, print_service, print_services};
pub use traces::{SpansArgs, TraceArgs, TracesArgs, print_spans, print_trace, print_traces};

const QUERY_HELP: &str = "The records to keep, such as 'http.route = \"/matches\" OR user.id = 7'. \
    Built-in fields come first; resource.<key> reads a resource attribute, and attr.<key> an \
    attribute named like a built-in field. Quote it for the shell";

/// The time range and the row limit of every query.
#[derive(clap::Args)]
pub struct RangeArgs {
    /// Start of the range: a duration before now, such as 1h, 30m, or 2d, or an
    /// RFC 3339 timestamp [default: 1h]
    #[arg(long)]
    since: Option<String>,

    /// End of the range, in the form of --since [default: now]
    #[arg(long)]
    until: Option<String>,

    /// The most rows to print
    #[arg(long)]
    limit: Option<usize>,
}

impl RangeArgs {
    fn to_query_params(&self) -> Vec<(&'static str, Option<String>)> {
        let mut params = self.to_range_params();
        params.push(("limit", self.limit.map(|limit| limit.to_string())));
        params
    }

    fn to_range_params(&self) -> Vec<(&'static str, Option<String>)> {
        vec![("since", self.since.clone()), ("until", self.until.clone())]
    }

    // The daemon answers at most a page, so a larger --limit takes several requests.
    fn fetch_pages<A: PagedAnswer>(
        &self,
        client: &Client,
        path: &str,
        params: &[(&'static str, Option<String>)],
    ) -> anyhow::Result<A> {
        let mut first_params = self.to_range_params();
        first_params.extend_from_slice(params);
        let Some(limit) = self.limit else {
            return client.get(path, &first_params);
        };
        let page_limit = |remaining: usize| Some(remaining.min(MAX_PAGE_ROWS).to_string());
        let mut answer: A = client.get(
            path,
            &[&first_params[..], &[("limit", page_limit(limit))]].concat(),
        )?;
        while let Some(after) = answer.next_cursor()
            && answer.row_count() < limit
        {
            let page_params = [
                ("limit", page_limit(limit - answer.row_count())),
                ("after", Some(after.to_string())),
            ];
            answer.append_page(client.get(path, &[&first_params[..], &page_params].concat())?);
        }
        Ok(answer)
    }
}

trait PagedAnswer: DeserializeOwned {
    fn next_cursor(&self) -> Option<PageCursor>;

    fn row_count(&self) -> usize;

    fn append_page(&mut self, page: Self);
}

impl PagedAnswer for Spans {
    fn next_cursor(&self) -> Option<PageCursor> {
        self.next.clone()
    }

    fn row_count(&self) -> usize {
        self.spans.len()
    }

    fn append_page(&mut self, page: Self) {
        self.spans.extend(page.spans);
        self.next = page.next;
    }
}

impl PagedAnswer for Traces {
    fn next_cursor(&self) -> Option<PageCursor> {
        self.next.clone()
    }

    fn row_count(&self) -> usize {
        self.traces.len()
    }

    fn append_page(&mut self, page: Self) {
        self.traces.extend(page.traces);
        self.next = page.next;
    }
}

impl PagedAnswer for Logs {
    fn next_cursor(&self) -> Option<PageCursor> {
        self.next.clone()
    }

    fn row_count(&self) -> usize {
        self.logs.len()
    }

    fn append_page(&mut self, page: Self) {
        self.logs.extend(page.logs);
        self.next = page.next;
    }
}

fn join_query_words(words: &[String]) -> Option<String> {
    (!words.is_empty()).then(|| words.join(" "))
}

fn note_unindexed_keys(signal: Signal, keys: &[String]) {
    if let Some(first_key) = keys.first() {
        let verb = if keys.len() == 1 { "has" } else { "have" };
        eprintln!(
            "{} {verb} no index, so the query read every record in the range; \
             `otelo index add {signal} {first_key}` adds one.",
            keys.join(", ")
        );
    }
}
