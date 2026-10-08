import { type SearchParams, useNavigate, useSearchParams } from "@solidjs/router";
import { createMemo, createSignal, Errored, Show, useContext } from "solid-js";

import type { DashboardSummary } from "@otelo/api";
import { PlusIcon } from "@otelo/icons";
import { Button, Callout } from "@otelo/ui";
import { type Column, Panel, Table } from "@otelo/viz";

import { ApiContext } from "../api";
import { pageContent, textField } from "../classes";
import { formatCount } from "../count";
import { createFetch, describeError } from "../fetch";
import FetchErrorBoundary from "../FetchErrorBoundary";
import PageBar from "../PageBar";
import { formatAge, parseTime } from "../time";
import { NEW_DASHBOARD_STATE } from "./new-dashboard";

const COLUMNS: Column<DashboardSummary>[] = [
  {
    kind: "cell",
    id: "name",
    label: "Name",
    width: "minmax(0,1fr)",
    sortBy: (dashboard) => dashboard.name.toLowerCase(),
    cell: (dashboard) => (
      <div class="min-w-0 font-sans">
        <div class="truncate font-semibold text-ink">{dashboard.name}</div>
        <Show when={dashboard.description}>
          <div class="truncate text-xs text-muted">{dashboard.description}</div>
        </Show>
      </div>
    ),
  },
  {
    kind: "number",
    id: "widgets",
    label: "Widgets",
    unit: "count",
    value: (dashboard) => dashboard.widget_count,
  },
  {
    kind: "cell",
    id: "updated",
    label: "Changed",
    width: "max-content",
    align: "end",
    sortBy: (dashboard) => dashboard.updated_at,
    cell: (dashboard) => (
      <span class="text-muted" title={dashboard.updated_at}>
        {formatAge(parseTime(dashboard.updated_at))}
      </span>
    ),
  },
];

interface DashboardSearchParams extends SearchParams {
  readonly q?: string;
}

export default function DashboardsPage() {
  const api = useContext(ApiContext);
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams<DashboardSearchParams>();
  const [createError, setCreateError] = createSignal<string>();
  const [creating, setCreating] = createSignal(false);
  const fetched = createFetch(
    "dashboards",
    () => ({}),
    (_, signal) => api.listDashboards(signal),
  );
  const search = () => params.q ?? "";
  const matchingDashboards = createMemo(() => {
    const words = search()
      .toLowerCase()
      .split(/\s+/)
      .filter((word) => word !== "");
    return (fetched.data()?.dashboards ?? []).filter((dashboard) => {
      const text = `${dashboard.name} ${dashboard.description}`.toLowerCase();
      return words.every((word) => text.includes(word));
    });
  });
  const createNewDashboard = async () => {
    setCreating(true);
    setCreateError(undefined);
    try {
      const dashboard = await api.createDashboard({
        name: "New dashboard",
        description: "",
        widgets: [],
      });
      navigate(`/dashboards/${dashboard.id}`, { state: NEW_DASHBOARD_STATE });
    } catch (error) {
      setCreateError(describeError(error));
      setCreating(false);
    }
  };

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <PageBar
        fetched={fetched}
        top={
          <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
            <h1 class="m-0 font-mono text-md font-semibold">Dashboards</h1>
            <input
              type="search"
              aria-label="Search the dashboards"
              placeholder="Search by name or description"
              class={`${textField} min-w-48 flex-1 basis-60 sm:max-w-md`}
              value={search()}
              onInput={(e) =>
                setParams({ q: e.currentTarget.value || undefined }, { replace: true })
              }
            />
            <Button
              variant="primary"
              class="ml-auto flex items-center gap-1.5"
              disabled={creating()}
              onClick={() => void createNewDashboard()}
            >
              <PlusIcon size={13} />
              New dashboard
            </Button>
          </div>
        }
        end={null}
      >
        <Errored fallback={null}>
          <Show when={fetched.data()}>
            {(list) => (
              <span>
                {search() === ""
                  ? formatCount(list().dashboards.length, "dashboard")
                  : `${matchingDashboards().length} of ${formatCount(list().dashboards.length, "dashboard")}`}
              </span>
            )}
          </Show>
        </Errored>
      </PageBar>

      <div class={`min-h-0 flex-1 ${pageContent}`}>
        <div>
          <Show when={createError() ?? fetched.errorMessage()}>
            {(errorMessage) => <Callout tone="error">{errorMessage()}</Callout>}
          </Show>
        </div>
        <FetchErrorBoundary>
          <Show when={fetched.data()}>
            {(list) => (
              <Show
                when={list().dashboards.length > 0}
                fallback={
                  <div class="flex flex-col items-center gap-3 py-16 text-center">
                    <p class="m-0 max-w-md text-muted">
                      A dashboard puts charts, numbers, and top lists of your spans, logs, and
                      metrics on one page.
                    </p>
                    <Button
                      variant="primary"
                      class="flex items-center gap-1.5"
                      disabled={creating()}
                      onClick={() => void createNewDashboard()}
                    >
                      <PlusIcon size={13} />
                      New dashboard
                    </Button>
                  </div>
                }
              >
                <Panel flush>
                  <Table
                    label="Dashboards"
                    rows={matchingDashboards()}
                    rowKey={(dashboard) => String(dashboard.id)}
                    columns={COLUMNS}
                    sorting={{
                      kind: "table",
                      initialOrder: { columnId: "updated", descending: true },
                    }}
                    href={(dashboard) => `/dashboards/${dashboard.id}`}
                    loading={fetched.loading()}
                    emptyMessage="No dashboard matches the search."
                  />
                </Panel>
              </Show>
            )}
          </Show>
        </FetchErrorBoundary>
      </div>
    </div>
  );
}
