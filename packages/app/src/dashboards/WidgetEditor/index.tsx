import { createSignal, Loading, Show, untrack } from "solid-js";

import type { Widget, WidgetDisplay } from "@otelo/api";
import { Button, CloseButton, Modal, Select, Tabs } from "@otelo/ui";

import { fieldLabel, inlineField, textField } from "../../classes";
import type { RangeState } from "../../services/range";
import DashboardWidget from "../DashboardWidget";
import DashboardWidgetSkeleton from "../DashboardWidgetSkeleton";
import {
  CHART_KIND_OPTIONS,
  changeDisplayKind,
  describeWidgetQuery,
  describeWidgetTitle,
  DISPLAY_KIND_OPTIONS,
  listWidgetQueries,
  measureRowsHeightPx,
  TOPLIST_LIMIT_OPTIONS,
  TOPLIST_ORDER_OPTIONS,
} from "../widget";
import WidgetEditorQueries from "./widget-editor-queries";
import WidgetEditorQuery from "./widget-editor-query";

interface WidgetEditorProps {
  readonly heading: string;
  readonly widget: Widget;
  readonly range: Pick<RangeState, "since" | "until" | "live" | "setRange">;
  readonly onApply: (widget: Widget) => void;
  readonly onClose: () => void;
}

export default function WidgetEditor(props: WidgetEditorProps) {
  const [widget, setWidget] = createSignal<Widget>(untrack(() => props.widget));
  const display = () => widget().display;
  const changeDisplay = (changed: WidgetDisplay) =>
    setWidget((current) => ({ ...current, display: changed }));
  const timeseries = () => {
    const shown = display();
    return shown.kind === "timeseries" ? shown : undefined;
  };
  const value = () => {
    const shown = display();
    return shown.kind === "value" ? shown : undefined;
  };
  const toplist = () => {
    const shown = display();
    return shown.kind === "toplist" ? shown : undefined;
  };
  const note = () => {
    const shown = display();
    return shown.kind === "note" ? shown : undefined;
  };
  const titlePlaceholder = () => {
    const shownNote = note();
    if (shownNote) return "Note";
    const first = timeseries()?.queries[0]?.query ?? value()?.query ?? toplist()?.query.query;
    return first ? describeWidgetQuery(first) : "";
  };
  const isMissingMetric = () =>
    listWidgetQueries(display()).some(
      ({ query }) => query.signal === "metrics" && query.name.trim() === "",
    );

  return (
    <Modal label={props.heading} onClose={props.onClose}>
      <header class="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b border-line py-2.5 pr-4 pl-2.5">
        <input
          aria-label="Title"
          class={`${inlineField} h-8 min-w-48 flex-1 text-md font-semibold`}
          value={widget().title}
          placeholder={titlePlaceholder()}
          onInput={(e) => {
            const title = e.currentTarget.value;
            setWidget((current) => ({ ...current, title }));
          }}
        />
        <Tabs
          label="Visualization"
          options={DISPLAY_KIND_OPTIONS}
          value={display().kind}
          onChange={(kind) => changeDisplay(changeDisplayKind(display(), kind))}
        />
        <CloseButton onClose={props.onClose} />
      </header>
      <div class="grid min-h-0 flex-1 grid-cols-1 overflow-y-auto lg:grid-cols-[minmax(0,1fr)_minmax(22rem,30rem)] lg:overflow-hidden">
        <section aria-label="Preview" class="bg-page p-4 lg:overflow-y-auto">
          <div style={{ height: `${measureRowsHeightPx(widget().layout.height)}px` }}>
            <Loading
              on={widget().display}
              fallback={<DashboardWidgetSkeleton title={describeWidgetTitle(widget())} />}
            >
              <DashboardWidget widget={widget()} range={props.range} />
            </Loading>
          </div>
        </section>
        <form
          class="flex flex-col gap-4 border-t border-line p-4 lg:overflow-y-auto lg:border-t-0 lg:border-l"
          onSubmit={(e) => {
            e.preventDefault();
            if (!isMissingMetric()) props.onApply(widget());
          }}
        >
          <Show when={timeseries()}>
            {(shown) => (
              <Tabs
                label="Chart"
                options={CHART_KIND_OPTIONS}
                value={shown().chart}
                onChange={(chart) => changeDisplay({ ...shown(), chart })}
              />
            )}
          </Show>
          <Show when={toplist()}>
            {(shown) => (
              <div class="flex flex-wrap items-center gap-2">
                <Select
                  selection="single"
                  label="Groups"
                  options={TOPLIST_LIMIT_OPTIONS}
                  value={String(shown().limit)}
                  onChange={(limit) => changeDisplay({ ...shown(), limit: Number(limit) })}
                />
                <Select
                  selection="single"
                  label="Order"
                  options={TOPLIST_ORDER_OPTIONS}
                  value={shown().order ?? "highest"}
                  onChange={(order) => changeDisplay({ ...shown(), order })}
                />
              </div>
            )}
          </Show>

          <Show when={timeseries()}>
            {(shown) => (
              <WidgetEditorQueries
                queries={shown().queries}
                range={props.range}
                onChange={(queries) => changeDisplay({ ...shown(), queries: [...queries] })}
              />
            )}
          </Show>
          <Show when={value()}>
            {(shown) => (
              <WidgetEditorQuery
                grouped={{ query: shown().query, by: [] }}
                groupable={false}
                range={props.range}
                onChange={(changed) => changeDisplay({ ...shown(), query: changed.query })}
              />
            )}
          </Show>
          <Show when={toplist()}>
            {(shown) => (
              <WidgetEditorQuery
                grouped={shown().query}
                groupable
                range={props.range}
                onChange={(query) => changeDisplay({ ...shown(), query })}
              />
            )}
          </Show>
          <Show when={note()}>
            {(shown) => (
              <label class="flex flex-col gap-1.5">
                <span class={fieldLabel}>Text</span>
                <textarea
                  class={`${textField} h-40 resize-y py-2`}
                  value={shown().text}
                  onInput={(e) => changeDisplay({ ...shown(), text: e.currentTarget.value })}
                />
              </label>
            )}
          </Show>

          <div class="mt-auto flex items-center justify-end gap-2 pt-2">
            <Show when={isMissingMetric()}>
              <span class="mr-auto text-muted">Pick a metric for each query.</span>
            </Show>
            <Button onClick={() => props.onClose()}>Cancel</Button>
            <Button type="submit" variant="primary" disabled={isMissingMetric()}>
              Apply
            </Button>
          </div>
        </form>
      </div>
    </Modal>
  );
}
