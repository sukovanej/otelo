use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{OperationDetail, RequestBucket, Requests, TraceSpan};
use crate::{Attributes, SpanKind};

/// The calls a service made in a range, by what they went to.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Calls {
    pub service: String,
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub end_at: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    /// Every call of the service.
    pub calls: Requests,
    /// The calls of every step, oldest first.
    pub buckets: Vec<RequestBucket>,
    /// What the service called, the most time first.
    pub targets: Vec<Target>,
    /// The targets have more operations than the limit let through, which
    /// kept the ones with the most time.
    pub truncated: bool,
}

/// What kind of thing a call goes to.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum TargetType {
    Database,
    Http,
    Rpc,
    Messaging,
    Other,
}

/// What a call goes to: a database, a host, an RPC service, or a message
/// destination.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct TargetKey {
    #[serde(rename = "type")]
    pub target_type: TargetType,
    /// Such as `postgresql`, `grpc`, or `kafka`. Null for HTTP, and for
    /// a target of no known type.
    #[schema(required = true)]
    pub system: Option<String>,
    /// The database, the host with a port that is not 80 or 443, the RPC
    /// service, the destination, or the `peer.service` of other calls.
    /// Null when the call does not say.
    #[schema(required = true)]
    pub name: Option<String>,
}

/// The calls to one target.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Target {
    #[serde(flatten)]
    pub key: TargetKey,
    /// The terms of a span query that keep the calls to the target, such as
    /// `db.system.name = "postgresql"`, from the attributes of one of them.
    /// Empty when no attribute tells the target.
    pub query: String,
    pub calls: Requests,
    /// The calls of every step, oldest first.
    pub buckets: Vec<RequestBucket>,
    /// The calls by summary and kind, the most time first.
    pub operations: Vec<CallOperation>,
}

/// The calls to a target that do the same thing.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CallOperation {
    /// What the calls do: `db.query.summary`, or the query with its values
    /// as `?`, such as `SELECT * FROM users WHERE id = ?`, for a database;
    /// the method and `url.template`, or the path with its ids as `{id}`,
    /// such as `GET /users/{id}`, for HTTP; and the span name for the rest.
    pub summary: String,
    /// The span name of its newest call.
    pub name: String,
    /// The OpenTelemetry span kind.
    #[schema(value_type = i32)]
    pub kind: SpanKind,
    /// The attributes of its newest call.
    pub attributes: Attributes,
    pub calls: Requests,
}

/// The calls of a service to one target that do one thing, over a range.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CallDetail {
    /// The calls, as the requests of an operation named after the span name
    /// of the newest one.
    #[serde(flatten)]
    pub operation: OperationDetail,
    pub target: TargetKey,
    /// What the calls do, as [`CallOperation::summary`] says.
    pub summary: String,
    /// The terms of a span query that keep these calls, or more of them when
    /// the summary comes from a query or a path with its values taken out.
    /// Empty without calls in the range.
    pub query: String,
    /// The newest calls, newest first.
    pub spans: Vec<TraceSpan>,
}

fn is_id_segment(segment: &str) -> bool {
    let only_hex_digits_and_dashes = segment
        .chars()
        .all(|character| character.is_ascii_hexdigit() || character == '-');
    !segment.is_empty()
        && (segment.chars().all(|character| character.is_ascii_digit())
            || (only_hex_digits_and_dashes
                && segment.len() >= 8
                && segment.chars().any(|character| character.is_ascii_digit())))
}

#[must_use]
pub fn replace_ids_in_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if is_id_segment(segment) {
                "{id}"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn starts_parameter(character: char, next_character: Option<char>) -> bool {
    next_character.is_some_and(|following| match character {
        ':' => following.is_ascii_alphabetic() || following == '_',
        _ => following.is_ascii_digit(),
    })
}

#[must_use]
pub fn replace_values_in_query(query: &str) -> String {
    let mut template = String::with_capacity(query.len());
    let mut chars = query.chars().peekable();
    let mut after_name_character = false;
    while let Some(character) = chars.next() {
        match character {
            '\'' => {
                while let Some(quoted) = chars.next() {
                    if quoted == '\'' && chars.next_if_eq(&'\'').is_none() {
                        break;
                    }
                }
                template.push('?');
                after_name_character = false;
            }
            '"' | '`' => {
                template.push(character);
                for quoted in chars.by_ref() {
                    template.push(quoted);
                    if quoted == character {
                        break;
                    }
                }
                after_name_character = true;
            }
            '$' | '?' | ':'
                if !after_name_character && starts_parameter(character, chars.peek().copied()) =>
            {
                while chars
                    .next_if(|following| following.is_ascii_alphanumeric() || *following == '_')
                    .is_some()
                {}
                template.push('?');
            }
            _ if character.is_ascii_digit() && !after_name_character => {
                while chars
                    .next_if(|following| following.is_ascii_alphanumeric() || *following == '.')
                    .is_some()
                {}
                template.push('?');
            }
            _ if character.is_whitespace() => {
                if !template.is_empty() && !template.ends_with(' ') {
                    template.push(' ');
                }
                after_name_character = false;
            }
            _ => {
                template.push(character);
                after_name_character =
                    character.is_alphanumeric() || matches!(character, '_' | '$' | '?' | ':' | '@');
            }
        }
    }
    let mut template = template.trim_end().to_owned();
    for (list, one) in [
        ("?, ?", "?"),
        ("?,?", "?"),
        ("(?), (?)", "(?)"),
        ("(?),(?)", "(?)"),
    ] {
        while template.contains(list) {
            template = template.replace(list, one);
        }
    }
    template
}
