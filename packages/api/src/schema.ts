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
        get: operations["attributes"];
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
         *     cursor of a query, from the attributes and values of the retention.
         */
        get: operations["complete"];
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
         * Indexes an attribute of the logs or of the spans in every day file, so a
         *     query that compares it reads only the matching records. The writer builds
         *     the index within seconds.
         */
        put: operations["add_index"];
        post?: never;
        /** Drops the index of an attribute from every day file. */
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
        get: operations["logs"];
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
        get: operations["log_groups"];
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
         *     name. The query reads `name`, `service`, `kind`, `unit`, the labels, and
         *     the resource.
         */
        get: operations["metrics"];
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
         * The series of one metric, each in buckets of one step with the count, the
         *     minimum, the average, the maximum, and the last value.
         */
        get: operations["metric"];
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
        get: operations["services"];
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
        get: operations["service"];
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
        get: operations["operation"];
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
        get: operations["spans"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/sql": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Runs a read-only SQL query over the day files of the range. */
        post: operations["sql"];
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
        get: operations["traces"];
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
        get: operations["trace"];
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
            type: string;
        };
        /** @description The attribute keys of a signal, over the attached days. */
        AttributeKeys: {
            /**
             * @description The attributes of the records: of the logs, the spans, or the labels
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
        AttributeValue: null | boolean | number | string | unknown[] | components["schemas"]["Attributes"];
        /** @description Attributes by key, in the order of their keys. */
        Attributes: {
            [key: string]: components["schemas"]["AttributeValue"];
        };
        /** @description The points of one series in one step. */
        Bucket: {
            /** Format: double */
            avg: number;
            /** Format: int64 */
            count: number;
            histogram: null | components["schemas"]["Distribution"];
            /**
             * Format: double
             * @description The value of the newest point.
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
            time: string;
        };
        /**
         * @description What the text of a suggestion is.
         * @enum {string}
         */
        CompletionKind: "field" | "operator" | "value" | "keyword";
        /** @description What can go at the cursor of a query. */
        Completions: {
            suggestions: components["schemas"]["SuggestionBody"][];
        };
        /**
         * @description The values a histogram series recorded in one time step: its points
         *     merged, with the increases of cumulative points.
         */
        Distribution: {
            /** @description The upper bounds of the buckets. The last bucket has no upper bound. */
            bounds: number[];
            /** Format: int64 */
            count: number;
            counts: number[];
            /**
             * Format: double
             * @description Estimates of the median and the 90th and 99th percentiles, by linear
             *     interpolation inside a bucket. `None` without values.
             */
            p50: number | null;
            /** Format: double */
            p90: number | null;
            /** Format: double */
            p99: number | null;
            /** Format: double */
            sum: number | null;
        };
        /** @description The body of every error response. */
        ErrorBody: {
            error: string;
        };
        IndexBody: {
            key: string;
            /** @description `logs` or `spans`. */
            signal: components["schemas"]["Signal"];
        };
        /** @description The attributes that have an index. */
        IndexList: {
            indexes: components["schemas"]["IndexBody"][];
        };
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
            first: string;
            /** Format: date-time */
            last: string;
            level: string;
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
            /** @description The name of the severity: TRACE, DEBUG, INFO, WARN, ERROR, or FATAL. */
            level: string;
            /** @description The attributes of the resource that sent the line. */
            resource: components["schemas"]["Attributes"];
            service: string;
            /**
             * Format: int32
             * @description The OpenTelemetry severity number.
             */
            severity: number;
            /** @description `otlp`, or the service log source that read the line. */
            source: string;
            span_id: string | null;
            /** Format: date-time */
            time: string;
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
        /** @description The series in a range, by name. */
        MetricList: {
            series: components["schemas"]["SeriesInfo"][];
            /** @description More series exist than the limit let through. */
            truncated: boolean;
        };
        /** @description The series of one metric, each in buckets of one step. */
        MetricSeries: {
            name: string;
            series: components["schemas"]["Series"][];
            /** Format: int64 */
            step_ns: number;
            /** @description More series match than the limit let through. */
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
            since: string;
            /**
             * Format: int64
             * @description The length of a bucket in nanoseconds.
             */
            step_ns: number;
            /** Format: date-time */
            until: string;
        };
        /** @description The requests of one step. */
        RequestBucket: {
            requests: components["schemas"]["Requests"];
            /**
             * Format: date-time
             * @description The start of the step.
             */
            time: string;
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
        Series: {
            /** @description The buckets that have points, oldest first. */
            buckets: components["schemas"]["Bucket"][];
            kind: string;
            labels: components["schemas"]["Attributes"];
            resource: components["schemas"]["Attributes"];
            service: string;
            unit: string;
        };
        SeriesInfo: {
            /** @description `gauge`, `sum`, or `histogram`. */
            kind: string;
            labels: components["schemas"]["Attributes"];
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
            since: string;
            stats: components["schemas"]["ServiceStats"];
            /**
             * Format: int64
             * @description The length of a bucket in nanoseconds.
             */
            step_ns: number;
            /** @description The service has more operations than the limit let through. */
            truncated: boolean;
            /** Format: date-time */
            until: string;
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
            time: string;
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
            services: components["schemas"]["ServiceSummary"][];
            /**
             * Format: date-time
             * @description The range, after the retention capped it.
             */
            since: string;
            /**
             * Format: int64
             * @description The length of a bucket in nanoseconds.
             */
            step_ns: number;
            /** @description More services sent telemetry than the limit let through. */
            truncated: boolean;
            /** Format: date-time */
            until: string;
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
            ts: number;
        };
        /** @description Spans, newest first. */
        Spans: {
            spans: components["schemas"]["TraceSpan"][];
            /** @description More spans match than the limit let through. */
            truncated: boolean;
            /** @description The attributes the query compares that have no index. */
            unindexed: string[];
        };
        /** @description A read-only SQL query. */
        SqlRequest: {
            /** @description The most rows to return. */
            limit?: number | null;
            /**
             * @description The start of the range, which chooses the day files: a duration before
             *     now, such as `1h`, or an RFC 3339 timestamp. One hour before `until`
             *     when missing.
             */
            since?: string | null;
            /**
             * @description One `SELECT`. It reads the views `resources`, `logs`, `spans`,
             *     `series`, `points`, `attribute_keys`, and `attribute_values`, which
             *     join the day files with a `day` column in front, or the tables of one
             *     day as `"2026-09-28".logs`.
             */
            sql: string;
            /** @description The end of the range, in the form of `since`. Now when missing. */
            until?: string | null;
        };
        SqlResult: {
            columns: string[];
            /** @description Each row has one value per column. */
            rows: components["schemas"]["SqlValue"][][];
            /** @description The query returned more rows than the limit let through. */
            truncated: boolean;
        };
        /** @description A value of a SQLite column. A blob is its hex digits, as text. */
        SqlValue: null | number | string;
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
            error: boolean;
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
            /**
             * Format: int32
             * @description The OpenTelemetry status code.
             */
            status: number;
            /** Format: date-time */
            time: string;
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
            time: string;
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
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    attributes: {
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
    complete: {
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
                signal: components["schemas"]["Signal"];
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
                signal: components["schemas"]["Signal"];
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
    logs: {
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
    log_groups: {
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
    metrics: {
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
    metric: {
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
                 * @description The series to keep, by their labels and resource, such as
                 *     `state = used`. Every series of the metric when missing.
                 */
                q?: string;
                /**
                 * @description The length of a bucket, such as `1m`. One that makes 120 buckets at
                 *     most when missing.
                 */
                step?: string;
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
    services: {
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
    service: {
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
    operation: {
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
    spans: {
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
    sql: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SqlRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SqlResult"];
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
    traces: {
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
    trace: {
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
