// Generated from openapi.json by generate.ts; `mise run api:generate`
// writes it again. Do not edit it by hand.

export interface paths {
    "/api/attributes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * The attribute keys of the records of a signal and of their resources over
         *     the retention, the most common first, with their types.
         */
        get: operations["list_attribute_keys"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/complete": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Suggests the fields, operators, values, and keywords that can go at the
         *     cursor of a query, from the attributes and values of the retention, and
         *     describes the field of the term the cursor is in.
         */
        get: operations["complete_query"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/indexes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** The attributes that have an index. */
        get: operations["list_indexes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/indexes/{signal}/{key}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Indexes an attribute of the logs or of the spans, so a query that
         *     compares it reads only the matching records. The writer builds
         *     the index within seconds.
         */
        put: operations["add_index"];
        post?: never;
        /** Drops the index of an attribute. */
        delete: operations["remove_index"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/logs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Log lines, newest first. */
        get: operations["list_logs"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/logs/groups": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Log lines grouped by message template, the largest group first. The
         *     template replaces numbers, UUIDs, hex IDs, and quoted strings with
         *     placeholders.
         */
        get: operations["list_log_groups"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/metrics": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * The series that have points in the range and that the query keeps, by
         *     name. The query reads `name`, `service`, `kind`, `unit`, the attributes, and
         *     the resource.
         */
        get: operations["list_metrics"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/metrics/{name}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * The series of one metric, or the groups of them, each in buckets of one
         *     step with the count, the minimum, the average, the maximum, and the last
         *     value, the rate of a counter, and the distribution of a histogram.
         */
        get: operations["get_metric_series"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/services": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * The services that sent spans or logs in the range, the most requests
         *     first. A request is a span that enters the service: a root span, or a
         *     span of the server or the consumer kind. Each service has its requests and
         *     its logs over the range and in buckets of one step.
         */
        get: operations["list_services"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/services/{name}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * One service: its requests and logs over the range and in buckets of one
         *     step, and its requests by span name, the most first. A service without
         *     telemetry in the range has none of either.
         */
        get: operations["get_service"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/services/{name}/call": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * The calls of a service to one target that do one thing, of one kind:
         *     over the range and in buckets of one step, the attributes of the newest
         *     one, and the newest 50. A call without spans in the range has none.
         */
        get: operations["get_call"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/services/{name}/calls": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * The calls a service makes, by what they go to: a span of the client or
         *     the producer kind, or of a database system. A target is a database, a
         *     host, an RPC service, or a message destination, read from the
         *     OpenTelemetry attributes of the call. Each target has its calls over the
         *     range and in buckets of one step, and by span name and kind, the most time
         *     first. What a call does is its query with the values taken out, the
         *     method and the path of an HTTP request with its ids taken out, or else
         *     its span name.
         */
        get: operations["list_calls"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/services/{name}/operation": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * One operation of a service: its requests over the range and in buckets of
         *     one step, and the attributes of its newest request. An operation without
         *     requests in the range has none.
         */
        get: operations["get_operation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/spans": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Spans, newest first. */
        get: operations["list_spans"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/traces": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Traces with a span that the query keeps, by their root span, newest first. */
        get: operations["list_traces"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/traces/{trace_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** One trace: its spans by start time, and the logs that carry its ID. */
        get: operations["get_trace"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        Attribute: {
            /**
             * Format: int64
             * @description How many records have the key. For resources and series, how many
             *     resources and series.
             */
            count: number;
            /** @description Whether the key has an index. */
            indexed: boolean;
            key: string;
            /**
             * @description The JSON type of the values: `string`, `int`, `float`, `bool`,
             *     `array`, `object`, or `mixed`.
             */
            type: components["schemas"]["ValueType"];
        };
        /** @description The attribute keys of a signal, over the retention of the signal. */
        AttributeKeys: {
            /**
             * @description The attributes of the records: of the logs, the spans, or the attributes
             *     of the series.
             */
            record: components["schemas"]["Attribute"][];
            /** @description The attributes of the resources that sent them. */
            resource: components["schemas"]["Attribute"][];
        };
        /**
         * @description The value of an attribute: the `AnyValue` of OpenTelemetry.
         *
         *     Bytes arrive as a base64 string, as OTLP/JSON writes them, so they read
         *     back as a string. A double that JSON cannot hold, such as NaN, is a string
         *     too.
         */
        AttributeValue: null | boolean | number | string | ArrayOfAttributeValue | components["schemas"]["Attributes"];
        /** @description Attributes by key, in the order of their keys. */
        Attributes: {
            [key: string]: components["schemas"]["AttributeValue"];
        };
        /** @description The points of one series or group in one step. */
        Bucket: {
            /** Format: double */
            avg: number;
            change: components["schemas"]["BucketChange"];
            /** Format: int64 */
            count: number;
            /**
             * Format: double
             * @description The value of the newest point. In a group, the newest points of its
             *     series combined.
             */
            last: number;
            /** Format: double */
            max: number;
            /** Format: double */
            min: number;
            /**
             * Format: date-time
             * @description The start of the bucket.
             */
            start_at: string;
        };
        /**
         * @description What the points of a step say beyond their values.
         *
         *     `none` for a gauge and an updown, and for a step with only the first
         *     point of a cumulative series, which has nothing to count from. `rate` is
         *     how much a counter grew per second: its increase between neighbouring
         *     points, over the time between them. A value that goes down is a restart,
         *     and the increase counts from zero. `distribution` is the merged buckets
         *     of a histogram with percentile estimates.
         */
        BucketChange: {
            /** @enum {string} */
            kind: "none";
        } | {
            /** @enum {string} */
            kind: "rate";
            /** Format: double */
            per_second: number;
        } | (components["schemas"]["Distribution"] & {
            /** @enum {string} */
            kind: "distribution";
        });
        /** @description The calls of a service to one target that do one thing, over a range. */
        CallDetail: components["schemas"]["OperationDetail"] & {
            /**
             * @description The terms of a span query that keep these calls, or more of them when
             *     the summary comes from a query or a path with its values taken out.
             *     Empty without calls in the range.
             */
            query: string;
            /** @description The newest calls, newest first. */
            spans: components["schemas"]["TraceSpan"][];
            /** @description What the calls do, as [`CallOperation::summary`] says. */
            summary: string;
            target: components["schemas"]["TargetKey"];
        };
        /** @description The calls to a target that do the same thing. */
        CallOperation: {
            /** @description The attributes of its newest call. */
            attributes: components["schemas"]["Attributes"];
            calls: components["schemas"]["Requests"];
            /**
             * Format: int32
             * @description The OpenTelemetry span kind.
             */
            kind: number;
            /** @description The span name of its newest call. */
            name: string;
            /**
             * @description What the calls do: `db.query.summary`, or the query with its values
             *     as `?`, such as `SELECT * FROM users WHERE id = ?`, for a database;
             *     the method and `url.template`, or the path with its ids as `{id}`,
             *     such as `GET /users/{id}`, for HTTP; and the span name for the rest.
             */
            summary: string;
        };
        /** @description The calls a service made in a range, by what they went to. */
        Calls: {
            /** @description The calls of every step, oldest first. */
            buckets: components["schemas"]["RequestBucket"][];
            /** @description Every call of the service. */
            calls: components["schemas"]["Requests"];
            /** Format: date-time */
            end_at: string;
            service: string;
            /**
             * Format: date-time
             * @description The range, after the retention capped it.
             */
            start_at: string;
            /**
             * Format: int64
             * @description The length of a bucket in nanoseconds.
             */
            step_ns: number;
            /** @description What the service called, the most time first. */
            targets: components["schemas"]["Target"][];
            /**
             * @description The targets have more operations than the limit let through, which
             *     kept the ones with the most time.
             */
            truncated: boolean;
        };
        /**
         * @description What the text of a suggestion is.
         * @enum {string}
         */
        CompletionKind: "field" | "operator" | "value" | "keyword";
        /** @description What can go at the cursor of a query, and what the field there holds. */
        Completions: {
            field: null | components["schemas"]["FieldBody"];
            suggestions: components["schemas"]["SuggestionBody"][];
        };
        /**
         * @description The values a histogram series recorded in one time step: its points
         *     merged, with the increases of cumulative points.
         */
        Distribution: {
            /**
             * @description The upper bounds of the buckets. The last bucket has no upper bound.
             *     An exponential histogram gets the bounds of its buckets, joined until
             *     64 of them are left.
             */
            bounds: number[];
            /** Format: int64 */
            count: number;
            counts: number[];
            percentiles: null | components["schemas"]["Percentiles"];
            /** Format: double */
            sum: number | null;
        };
        /** @description The body of every error response. */
        ErrorBody: {
            error: string;
        };
        /** @description A field of a query: where it comes from, its type, and its values. */
        FieldBody: components["schemas"]["FieldOriginBody"] & {
            /** @description How many distinct values the daemon knows. */
            distinct_values: number;
            /**
             * @description Whether the field has more distinct values than the daemon keeps, so
             *     `distinct_values` counts only some of them.
             */
            has_more_values_than_listed: boolean;
            /** @description The field as a query writes it. */
            name: string;
            /**
             * @description The type of the values. Of an attribute: `string`, `int`, `float`,
             *     `bool`, `array`, `object`, or `mixed`. Of a built-in field: `string`,
             *     `bool`, or `duration`.
             */
            type: components["schemas"]["ValueType"];
            /**
             * @description The values, the most common first, and 10 at most. For a built-in
             *     field with fixed values, those, in their order.
             */
            values: components["schemas"]["FieldValueBody"][];
        };
        /**
         * @description Where a field comes from: the query language, the attributes of the
         *     records, or the attributes of their resources.
         */
        FieldOriginBody: {
            /** @description What the field holds. */
            description: string;
            /** @enum {string} */
            source: "builtin";
        } | {
            /**
             * Format: int64
             * @description How many records have the attribute.
             */
            count: number;
            /** @enum {string} */
            source: "attribute";
        } | {
            /**
             * Format: int64
             * @description How many resources have the attribute.
             */
            count: number;
            /** @enum {string} */
            source: "resource";
        };
        FieldValueBody: {
            /**
             * Format: int64
             * @description How many records have the value. Missing for the fixed values of a
             *     built-in field.
             */
            count: number | null;
            /** @description The value as a query writes it. */
            text: string;
        };
        /**
         * @description What a group holds.
         *
         *     `series` is one series, when the query groups by nothing. `values` is
         *     the series that have these values of the names of `by`, and a name that
         *     the series lack is missing. `other` is the groups past `top`.
         */
        GroupKey: {
            attributes: components["schemas"]["Attributes"];
            /** @description The attributes of the resource that sends the series. */
            resource: components["schemas"]["Attributes"];
            service: string;
            /** @enum {string} */
            type: "series";
        } | {
            series_count: number;
            /** @enum {string} */
            type: "values";
            values: components["schemas"]["Attributes"];
        } | {
            group_count: number;
            series_count: number;
            /** @enum {string} */
            type: "other";
        };
        IndexBody: {
            key: string;
            /** @description `logs` or `spans`. */
            signal: components["schemas"]["IndexedSignal"];
        };
        /** @description The attributes that have an index. */
        IndexList: {
            indexes: components["schemas"]["IndexBody"][];
        };
        /**
         * @description The kind of record whose attributes can have an index, as the API names
         *     it.
         * @enum {string}
         */
        IndexedSignal: "logs" | "spans";
        /** @description Percentiles of durations, in nanoseconds. */
        Latency: {
            /** Format: int64 */
            p50: number;
            /** Format: int64 */
            p95: number;
            /** Format: int64 */
            p99: number;
        };
        LogGroup: {
            /** Format: int64 */
            count: number;
            /** Format: date-time */
            first_at: string;
            /** Format: date-time */
            last_at: string;
            /** @description Up to three different bodies, newest first. */
            samples: string[];
            services: string[];
            /**
             * Format: int32
             * @description The highest severity number in the group.
             */
            severity: number;
            /**
             * @description The body with numbers, UUIDs, hex IDs, and quoted strings replaced by
             *     placeholders.
             */
            template: string;
        };
        /** @description Log lines grouped by message template, the largest group first. */
        LogGroups: {
            groups: components["schemas"]["LogGroup"][];
            /**
             * @description The range has more lines than the grouping reads, so the groups count
             *     only the newest ones.
             */
            partial: boolean;
            /**
             * Format: int64
             * @description How many lines the groups count.
             */
            scanned: number;
            /** @description More groups exist than the limit let through. */
            truncated: boolean;
            /** @description The attributes the query compares that have no index. */
            unindexed: string[];
        };
        LogLine: {
            attributes: components["schemas"]["Attributes"];
            body: string;
            /** Format: date-time */
            logged_at: string;
            /** @description The attributes of the resource that sent the line. */
            resource: components["schemas"]["Attributes"];
            service: string;
            /**
             * Format: int32
             * @description The OpenTelemetry severity number, from 1 (TRACE) to 24 (FATAL), or 0
             *     when the source set none.
             */
            severity: number;
            span_id: string | null;
            trace_id: string | null;
        };
        /** @description Log lines, newest first. */
        Logs: {
            logs: components["schemas"]["LogLine"][];
            /** @description More lines match than the limit let through. */
            truncated: boolean;
            /**
             * @description The attributes the query compares that have no index, so it read every
             *     line in the range.
             */
            unindexed: string[];
        };
        /**
         * @description How the points of a series combine over time.
         *
         *     A `gauge` is a value at an instant, such as a CPU share. An `updown` is a
         *     level that goes up and down, such as the memory in use, and the series of
         *     one metric add up. A `counter` is a total that only grows, such as the
         *     bytes sent, and a chart shows its rate. A `histogram` is the distribution
         *     of many values, such as request durations. A `counter` and a `histogram`
         *     have a temporality.
         */
        MetricKind: {
            /** @enum {string} */
            kind: "gauge";
        } | {
            /** @enum {string} */
            kind: "updown";
        } | {
            /** @enum {string} */
            kind: "counter";
            temporality: components["schemas"]["Temporality"];
        } | {
            /** @enum {string} */
            kind: "histogram";
            temporality: components["schemas"]["Temporality"];
        };
        /** @description The series in a range, by name. */
        MetricList: {
            series: components["schemas"]["SeriesInfo"][];
            /** @description More series exist than the limit let through. */
            truncated: boolean;
        };
        /**
         * @description The series of one metric, or the groups of them, each in buckets of one
         *     step.
         */
        MetricSeries: {
            /** Format: date-time */
            end_at: string;
            /**
             * @description The highest value over the range first: the average of a gauge and an
             *     updown, the rate of a counter, and the sum of the values of a
             *     histogram. The group `other` comes last.
             */
            groups: components["schemas"]["SeriesGroup"][];
            name: string;
            resolution: components["schemas"]["Resolution"];
            /**
             * Format: date-time
             * @description The range, after the retention capped it.
             */
            start_at: string;
            /**
             * Format: int64
             * @description The length of a bucket. A query of the summaries by the minute or by
             *     the hour rounds the step up to whole minutes or hours.
             */
            step_ns: number;
            /**
             * @description More groups match than the limit let through, or the metric has more
             *     series than a query reads.
             */
            truncated: boolean;
        };
        /** @description The requests of a service by the name of their span. */
        Operation: {
            /**
             * @description The attributes of its newest request, which tell what the operation
             *     is, such as `http.request.method` and `http.route`.
             */
            attributes: components["schemas"]["Attributes"];
            /**
             * Format: int32
             * @description The OpenTelemetry span kind.
             */
            kind: number;
            /** @description The span name, such as `GET /users/{id}`. */
            name: string;
            requests: components["schemas"]["Requests"];
        };
        /**
         * @description One operation of a service in a range: its requests over the range and
         *     in buckets of one step.
         */
        OperationDetail: {
            /** @description The attributes of its newest request. Empty when the range has none. */
            attributes: components["schemas"]["Attributes"];
            /** @description Every step of the range, oldest first. */
            buckets: components["schemas"]["RequestBucket"][];
            /** Format: date-time */
            end_at: string;
            /**
             * Format: int32
             * @description The OpenTelemetry span kind.
             */
            kind: number;
            /** @description The span name. */
            name: string;
            requests: components["schemas"]["Requests"];
            service: string;
            /**
             * Format: date-time
             * @description The range, after the retention capped it.
             */
            start_at: string;
            /**
             * Format: int64
             * @description The length of a bucket in nanoseconds.
             */
            step_ns: number;
        };
        /** @description Percentiles of the values a histogram recorded. */
        Percentiles: {
            /** Format: double */
            p50: number;
            /** Format: double */
            p90: number;
            /** Format: double */
            p99: number;
        };
        /** @description The requests of one step. */
        RequestBucket: {
            requests: components["schemas"]["Requests"];
            /**
             * Format: date-time
             * @description The start of the step.
             */
            start_at: string;
        };
        /**
         * @description Spans that enter a service: roots, and spans of the server or the
         *     consumer kind.
         */
        Requests: {
            /** Format: int64 */
            count: number;
            /**
             * Format: int64
             * @description The failed ones.
             */
            errors: number;
            latency: null | components["schemas"]["Latency"];
            /**
             * Format: int64
             * @description Their durations added up.
             */
            total_ns: number;
        };
        /**
         * @description Which points a metric query reads.
         *
         *     `raw` is the points as they arrived. `1m` and `1h` are their summaries
         *     by the minute and by the hour, which a long range reads in fewer rows.
         * @enum {string}
         */
        Resolution: "raw" | "1m" | "1h";
        /**
         * @description One series, or the series of a group combined in each step.
         *
         *     A gauge takes their average, an updown and a counter add up, and a
         *     histogram merges its buckets. The minimum and the maximum of series that
         *     add up are the sums of theirs, since their points do not line up in time.
         */
        SeriesGroup: components["schemas"]["MetricKind"] & {
            /** @description The buckets that have points, oldest first. */
            buckets: components["schemas"]["Bucket"][];
            key: components["schemas"]["GroupKey"];
            unit: string;
        };
        SeriesInfo: components["schemas"]["MetricKind"] & {
            attributes: components["schemas"]["Attributes"];
            name: string;
            /** @description The attributes of the resource that sends the series. */
            resource: components["schemas"]["Attributes"];
            service: string;
            unit: string;
        };
        /** @description One service in a range: its stats over time and by operation. */
        Service: {
            /** @description Every step of the range, oldest first. */
            buckets: components["schemas"]["ServiceBucket"][];
            /** Format: date-time */
            end_at: string;
            /** @description The requests by span name, the most first. */
            operations: components["schemas"]["Operation"][];
            /**
             * @description The attributes of the newest resource of the service. Empty when the
             *     range has none of its telemetry.
             */
            resource: components["schemas"]["Attributes"];
            service: string;
            /**
             * Format: date-time
             * @description The range, after the retention capped it.
             */
            start_at: string;
            stats: components["schemas"]["ServiceStats"];
            /**
             * Format: int64
             * @description The length of a bucket in nanoseconds.
             */
            step_ns: number;
            /** @description The service has more operations than the limit let through. */
            truncated: boolean;
        };
        /** @description The requests and the logs of a service in one step. */
        ServiceBucket: {
            /** Format: int64 */
            error_logs: number;
            /** Format: int64 */
            logs: number;
            requests: components["schemas"]["Requests"];
            /**
             * Format: date-time
             * @description The start of the step.
             */
            start_at: string;
        };
        /** @description The requests and the logs of a service in a range. */
        ServiceStats: {
            /**
             * Format: int64
             * @description The logs of the error level and above.
             */
            error_logs: number;
            /** Format: int64 */
            logs: number;
            requests: components["schemas"]["Requests"];
        };
        ServiceSummary: {
            /** @description Every step of the range, oldest first. */
            buckets: components["schemas"]["ServiceBucket"][];
            /**
             * @description The attributes of the newest resource of the service that has any,
             *     such as `telemetry.sdk.language`.
             */
            resource: components["schemas"]["Attributes"];
            service: string;
            stats: components["schemas"]["ServiceStats"];
        };
        /** @description The services that sent spans or logs in a range, the busiest first. */
        Services: {
            /** Format: date-time */
            end_at: string;
            services: components["schemas"]["ServiceSummary"][];
            /**
             * Format: date-time
             * @description The range, after the retention capped it.
             */
            start_at: string;
            /**
             * Format: int64
             * @description The length of a bucket in nanoseconds.
             */
            step_ns: number;
            /** @description More services sent telemetry than the limit let through. */
            truncated: boolean;
        };
        /**
         * @description The kind of record a query reads, as the API names it.
         * @enum {string}
         */
        Signal: "logs" | "spans" | "metrics";
        /** @description Something that happened at one time in a span, such as an exception. */
        SpanEvent: {
            attributes: components["schemas"]["Attributes"];
            name: string;
            /**
             * Format: int64
             * @description Nanoseconds since the Unix epoch.
             */
            occurred_at: number;
        };
        /** @description Spans, newest first. */
        Spans: {
            spans: components["schemas"]["TraceSpan"][];
            /** @description More spans match than the limit let through. */
            truncated: boolean;
            /** @description The attributes the query compares that have no index. */
            unindexed: string[];
        };
        SuggestionBody: {
            /**
             * @description The type of a field and how many records have it, or how many records
             *     have a value.
             */
            detail: string | null;
            end: number;
            kind: components["schemas"]["CompletionKind"];
            start: number;
            /** @description The text to put in place of the characters from `start` to `end`. */
            text: string;
        };
        /** @description The calls to one target. */
        Target: components["schemas"]["TargetKey"] & {
            /** @description The calls of every step, oldest first. */
            buckets: components["schemas"]["RequestBucket"][];
            calls: components["schemas"]["Requests"];
            /** @description The calls by summary and kind, the most time first. */
            operations: components["schemas"]["CallOperation"][];
            /**
             * @description The terms of a span query that keep the calls to the target, such as
             *     `db.system.name = "postgresql"`, from the attributes of one of them.
             *     Empty when no attribute tells the target.
             */
            query: string;
        };
        /**
         * @description What a call goes to: a database, a host, an RPC service, or a message
         *     destination.
         */
        TargetKey: {
            /**
             * @description The database, the host with a port that is not 80 or 443, the RPC
             *     service, the destination, or the `peer.service` of other calls.
             *     Null when the call does not say.
             */
            name: string | null;
            /**
             * @description Such as `postgresql`, `grpc`, or `kafka`. Null for HTTP, and for
             *     a target of no known type.
             */
            system: string | null;
            type: components["schemas"]["TargetType"];
        };
        /**
         * @description What kind of thing a call goes to.
         * @enum {string}
         */
        TargetType: "database" | "http" | "rpc" | "messaging" | "other";
        /**
         * @description What a point of a series counts: `cumulative` since the series started, or
         *     `delta` since the point before.
         * @enum {string}
         */
        Temporality: "cumulative" | "delta";
        /** @description One trace: its spans by start time, and the logs that carry its ID. */
        Trace: {
            /** @description Newest first. */
            logs: components["schemas"]["LogLine"][];
            spans: components["schemas"]["TraceSpan"][];
            trace_id: string;
            /** @description The trace has more spans or logs than the limit let through. */
            truncated: boolean;
        };
        TraceSpan: {
            attributes: components["schemas"]["Attributes"];
            /** Format: int64 */
            duration_ns: number;
            events: components["schemas"]["SpanEvent"][];
            /**
             * Format: int32
             * @description The OpenTelemetry span kind.
             */
            kind: number;
            name: string;
            parent_span_id: string | null;
            /** @description The attributes of the resource that sent the span. */
            resource: components["schemas"]["Attributes"];
            service: string;
            span_id: string;
            /** Format: date-time */
            started_at: string;
            /**
             * Format: int32
             * @description The OpenTelemetry status code: 2 when the span failed.
             */
            status: number;
            trace_id: string;
        };
        TraceSummary: {
            /** @description The attributes of the root span, such as `http.route`. */
            attributes: components["schemas"]["Attributes"];
            /** Format: int64 */
            duration_ns: number;
            /** @description Whether a span of the trace failed. */
            error: boolean;
            /**
             * Format: int32
             * @description The OpenTelemetry span kind of the root span.
             */
            kind: number;
            /** @description The name of the root span. */
            name: string;
            /** @description The attributes of the resource that sent the root span. */
            resource: components["schemas"]["Attributes"];
            service: string;
            /** Format: int64 */
            spans: number;
            /**
             * Format: date-time
             * @description The start of the root span.
             */
            started_at: string;
            trace_id: string;
        };
        /** @description Traces by their root span, newest first. */
        Traces: {
            traces: components["schemas"]["TraceSummary"][];
            /** @description More traces match than the limit let through. */
            truncated: boolean;
            /** @description The attributes the query compares that have no index. */
            unindexed: string[];
        };
        /**
         * @description The type of the values of a field. `mixed` when the values of an
         *     attribute have more than one type, and `duration` only for a built-in
         *     field.
         * @enum {string}
         */
        ValueType: "null" | "bool" | "int" | "float" | "string" | "array" | "object" | "mixed" | "duration";
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    list_attribute_keys: {
        parameters: {
            query: {
                signal: components["schemas"]["Signal"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AttributeKeys"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    complete_query: {
        parameters: {
            query: {
                signal: components["schemas"]["Signal"];
                /** @description The query as typed so far. */
                q?: string;
                /**
                 * @description The position of the cursor in the query, in characters. The end of the
                 *     query when missing.
                 */
                cursor?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Completions"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_indexes: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["IndexList"];
                };
            };
        };
    };
    add_index: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description `logs` or `spans` */
                signal: components["schemas"]["IndexedSignal"];
                /** @description The attribute key, such as `user.id` */
                key: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["IndexList"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    remove_index: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description `logs` or `spans` */
                signal: components["schemas"]["IndexedSignal"];
                /** @description The attribute key, such as `user.id` */
                key: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["IndexList"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_logs: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most rows to return. */
                limit?: number;
                /**
                 * @description The records to keep, such as `http.route = "/matches" OR user.id = 7`.
                 *     Every record when missing.
                 */
                q?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Logs"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_log_groups: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most rows to return. */
                limit?: number;
                /**
                 * @description The records to keep, such as `http.route = "/matches" OR user.id = 7`.
                 *     Every record when missing.
                 */
                q?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LogGroups"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_metrics: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most rows to return. */
                limit?: number;
                /**
                 * @description The records to keep, such as `http.route = "/matches" OR user.id = 7`.
                 *     Every record when missing.
                 */
                q?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MetricList"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    get_metric_series: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most series to return. */
                limit?: number;
                /**
                 * @description The series to keep, by their attributes and resource, such as
                 *     `state = used`. Every series of the metric when missing.
                 */
                q?: string;
                /**
                 * @description The length of a bucket, such as `1m`. One that makes 120 buckets at
                 *     most when missing.
                 */
                step?: string;
                /**
                 * @description Which points to read: `raw`, `1m`, or `1h`. When missing, the raw
                 *     points for a range of 6 hours at most, the summaries by the minute
                 *     for one of 14 days at most, and the summaries by the hour for a longer
                 *     one, or the next of them that is still kept where the range starts.
                 */
                resolution?: components["schemas"]["Resolution"];
                /**
                 * @description The names to group the series by, separated by commas: attributes,
                 *     `service`, or `resource.<key>`, such as `http.route,resource.host.name`.
                 *     The series with the same values of them combine into one group. Each
                 *     series is its own group when missing.
                 */
                by?: string;
                /**
                 * @description Keep the N groups with the highest value over the range, and combine
                 *     the rest into one group `other`.
                 */
                top?: number;
            };
            header?: never;
            path: {
                /** @description The name of the metric */
                name: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MetricSeries"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_services: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most services, or the most operations of one service, to return. */
                limit?: number;
                /**
                 * @description The length of a bucket, such as `1m`. One that makes 60 buckets at
                 *     most for the list and 120 for one service when missing.
                 */
                step?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Services"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    get_service: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most services, or the most operations of one service, to return. */
                limit?: number;
                /**
                 * @description The length of a bucket, such as `1m`. One that makes 60 buckets at
                 *     most for the list and 120 for one service when missing.
                 */
                step?: string;
            };
            header?: never;
            path: {
                /** @description The name of the service */
                name: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Service"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    get_call: {
        parameters: {
            query: {
                /** @description What the target is. */
                type: "database" | "http" | "rpc" | "messaging" | "other";
                /**
                 * @description The system of the target, such as `postgresql`. Missing for a
                 *     target without one.
                 */
                system?: string;
                /**
                 * @description The name of the target, such as a database or a host. Missing for a
                 *     target without one.
                 */
                target?: string;
                /**
                 * @description What the calls do, such as `SELECT * FROM users WHERE id = ?`, as
                 *     `/api/services/{name}/calls` names it.
                 */
                summary: string;
                /** @description The OpenTelemetry span kind of the call, such as 3 for client. */
                kind: number;
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /**
                 * @description The length of a bucket, such as `1m`. One that makes 120 buckets at
                 *     most when missing.
                 */
                step?: string;
            };
            header?: never;
            path: {
                /** @description The name of the service */
                name: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CallDetail"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_calls: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most services, or the most operations of one service, to return. */
                limit?: number;
                /**
                 * @description The length of a bucket, such as `1m`. One that makes 60 buckets at
                 *     most for the list and 120 for one service when missing.
                 */
                step?: string;
            };
            header?: never;
            path: {
                /** @description The name of the service */
                name: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Calls"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    get_operation: {
        parameters: {
            query: {
                /** @description The span name of the operation, such as `GET /users/{id}`. */
                operation: string;
                /** @description The OpenTelemetry span kind of the operation, such as 2 for server. */
                kind: number;
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /**
                 * @description The length of a bucket, such as `1m`. One that makes 120 buckets at
                 *     most when missing.
                 */
                step?: string;
            };
            header?: never;
            path: {
                /** @description The name of the service */
                name: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OperationDetail"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_spans: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most rows to return. */
                limit?: number;
                /**
                 * @description The records to keep, such as `http.route = "/matches" OR user.id = 7`.
                 *     Every record when missing.
                 */
                q?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Spans"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_traces: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. One hour before `until` when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most rows to return. */
                limit?: number;
                /**
                 * @description The records to keep, such as `http.route = "/matches" OR user.id = 7`.
                 *     Every record when missing.
                 */
                q?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Traces"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    get_trace: {
        parameters: {
            query?: {
                /**
                 * @description The start of the range: a duration before now, such as `1h`, or an
                 *     RFC 3339 timestamp. The whole retention when missing.
                 */
                since?: string;
                /** @description The end of the range, in the form of `since`. Now when missing. */
                until?: string;
                /** @description The most spans, and the most logs, to return. */
                limit?: number;
            };
            header?: never;
            path: {
                /** @description The trace ID as 32 hex digits */
                trace_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Trace"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
}
type ArrayOfAttributeValue = components["schemas"]["AttributeValue"][];
