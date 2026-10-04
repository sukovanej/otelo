import {
  type BeforeLeaveEventArgs,
  useBeforeLeave,
  useLocation,
  useNavigate,
} from "@solidjs/router";
import {
  createEffect,
  createMemo,
  createSignal,
  latest,
  onSettled,
  Show,
  snapshot,
  untrack,
} from "solid-js";

import {
  createDashboard,
  type DashboardDefinition,
  deleteDashboard,
  getDashboard,
  replaceDashboard,
  type Widget,
} from "@otelo/api";
import { CopyIcon, PlusIcon, TrashIcon } from "@otelo/icons";
import {
  ActionMenu,
  type ActionMenuItem,
  Button,
  Callout,
  ConfirmDialog,
  RangePicker,
} from "@otelo/ui";

import { inlineField, link, pageContent } from "../../classes";
import { createFetch, describeError, freezeDeeply } from "../../fetch";
import FetchErrorBoundary from "../../FetchErrorBoundary";
import LiveToggle from "../../LiveToggle";
import PageBar from "../../PageBar";
import { useRange } from "../../services/range";
import { placeBelow, placeBeside, settleLayouts } from "../layout";
import { isNewDashboardState } from "../new-dashboard";
import { createDefaultWidget, describeWidgetTitle } from "../widget";
import WidgetEditor from "../WidgetEditor";
import DashboardPageGrid from "./dashboard-page-grid";

type OpenEditor = NewWidgetEditor | ExistingWidgetEditor;

type PendingAction = "save" | "clone" | "delete";

interface NewWidgetEditor {
  readonly kind: "new";
}

interface ExistingWidgetEditor {
  readonly kind: "existing";
  readonly index: number;
  readonly widget: Widget;
}

interface PendingRemoval {
  readonly index: number;
  readonly title: string;
}

interface DashboardPageViewProps {
  readonly id: number;
}

export default function DashboardPageView(props: DashboardPageViewProps) {
  let nameInput!: HTMLInputElement;
  const navigate = useNavigate();
  const location = useLocation();
  const range = useRange();
  const isNewDashboard = untrack(() => isNewDashboardState(location.state));
  const fetched = createFetch(
    "dashboard",
    () => ({ id: props.id }),
    ({ id }, signal) => getDashboard(id, signal).then(freezeDeeply),
  );
  // The page edits its own copy, so a refetch never takes back a change that
  // is not saved yet. A refetch with no change pending replaces the copy.
  const [savedDefinition, setSavedDefinition] = createSignal<DashboardDefinition>();
  const [definition, setDefinition] = createSignal<DashboardDefinition>();
  let editedDefinition: DashboardDefinition | undefined;
  let lastSavedDefinition: DashboardDefinition | undefined;
  let isLeavingOnPurpose = false;
  let hasSelectedName = false;
  const [openEditor, setOpenEditor] = createSignal<OpenEditor>();
  const [actionError, setActionError] = createSignal<string>();
  const [pendingAction, setPendingAction] = createSignal<PendingAction>();
  const [pendingRemoval, setPendingRemoval] = createSignal<PendingRemoval>();
  const [pendingLeave, setPendingLeave] = createSignal<BeforeLeaveEventArgs>();
  const [deleting, setDeleting] = createSignal(false);

  const changeDefinition = (change: (current: DashboardDefinition) => DashboardDefinition) => {
    if (!editedDefinition) return;
    editedDefinition = change(editedDefinition);
    setDefinition(editedDefinition);
  };
  const adoptSavedDefinition = (saved: DashboardDefinition) => {
    editedDefinition = saved;
    lastSavedDefinition = saved;
    setDefinition(saved);
    setSavedDefinition(saved);
  };
  const hasChanges = createMemo(
    () => JSON.stringify(definition()) !== JSON.stringify(savedDefinition()),
  );
  const runAction = async (kind: PendingAction, action: () => Promise<void>) => {
    setPendingAction(kind);
    setActionError(undefined);
    try {
      await action();
      return true;
    } catch (error) {
      setActionError(describeError(error));
      return false;
    } finally {
      setPendingAction(undefined);
    }
  };
  const saveChanges = () =>
    runAction("save", async () => {
      if (!editedDefinition) return;
      const sent = editedDefinition;
      fetched.replaceData(freezeDeeply(await replaceDashboard(props.id, sent)));
      lastSavedDefinition = sent;
      setSavedDefinition(sent);
    });
  const discardChanges = () => {
    const saved = savedDefinition();
    if (!saved) return;
    editedDefinition = saved;
    setDefinition(saved);
    setActionError(undefined);
  };
  const changeWidgets = (change: (widgets: ReadonlyArray<Widget>) => Widget[]) =>
    changeDefinition((current) => ({ ...current, widgets: change(current.widgets) }));
  const applyEditedWidget = (editor: OpenEditor, widget: Widget) => {
    changeWidgets((widgets) => {
      if (editor.kind === "new") {
        const layouts = widgets.map((current) => current.layout);
        return [...widgets, { ...widget, layout: placeBelow(layouts, widget.layout) }];
      }
      return settleWidgets(
        widgets.map((current, index) => (index === editor.index ? widget : current)),
        editor.index,
      );
    });
    setOpenEditor(undefined);
  };
  const duplicateWidget = (index: number) =>
    changeWidgets((widgets) => {
      const widget = widgets[index];
      if (!widget) return [...widgets];
      const copy = { ...widget, layout: placeBeside(widget.layout) };
      return settleWidgets([...widgets, copy], widgets.length);
    });
  const removeWidget = (index: number) => {
    changeWidgets((widgets) => settleWidgets(widgets.toSpliced(index, 1), undefined));
    setPendingRemoval(undefined);
  };
  const cloneDashboard = () =>
    runAction("clone", async () => {
      if (!editedDefinition) return;
      const clone = await createDashboard({
        ...editedDefinition,
        name: `${editedDefinition.name} (copy)`,
      });
      isLeavingOnPurpose = true;
      navigate(`/dashboards/${clone.id}${range.toSearch()}`);
    });
  const deleteThisDashboard = async () => {
    const isDeleted = await runAction("delete", async () => {
      await deleteDashboard(props.id);
      isLeavingOnPurpose = true;
      navigate("/dashboards");
    });
    if (!isDeleted) setDeleting(false);
  };
  const saveWithKeys = (e: KeyboardEvent) => {
    if (e.key !== "s" || !(e.metaKey || e.ctrlKey)) return;
    e.preventDefault();
    if (hasChanges() && !pendingAction()) void saveChanges();
  };
  const dashboardMenuItems: ReadonlyArray<ActionMenuItem> = [
    {
      label: "Clone",
      icon: () => <CopyIcon size={14} />,
      tone: "default",
      onSelect: () => void cloneDashboard(),
    },
    {
      label: "Delete",
      icon: () => <TrashIcon size={14} />,
      tone: "danger",
      onSelect: () => setDeleting(true),
    },
  ];

  createEffect(
    () => {
      const saved = fetched.data();
      return saved && snapshot(saved.definition);
    },
    (fetchedDefinition) => {
      if (!fetchedDefinition) return;
      const isFirst = editedDefinition === undefined;
      const isUnchanged = JSON.stringify(editedDefinition) === JSON.stringify(lastSavedDefinition);
      if (!isFirst && !isUnchanged) return;
      adoptSavedDefinition(fetchedDefinition);
    },
  );
  createEffect(
    () => definition() !== undefined,
    (isLoaded) => {
      if (!isLoaded || !isNewDashboard || hasSelectedName) return;
      hasSelectedName = true;
      nameInput.focus();
      nameInput.select();
    },
  );
  createEffect(hasChanges, (unsaved) => {
    if (!unsaved) return undefined;
    window.addEventListener("beforeunload", warnAboutUnsavedChanges);
    return () => window.removeEventListener("beforeunload", warnAboutUnsavedChanges);
  });
  onSettled(() => {
    window.addEventListener("keydown", saveWithKeys);
    return () => window.removeEventListener("keydown", saveWithKeys);
  });
  useBeforeLeave((e) => {
    if (isLeavingOnPurpose || e.defaultPrevented || !hasChanges()) return;
    e.preventDefault();
    setPendingLeave(e);
  });

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <PageBar
        fetched={fetched}
        top={
          <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
            <div class="-ml-1.5 flex min-w-0 flex-1 basis-72 flex-col gap-1">
              <input
                ref={nameInput}
                aria-label="Name"
                class={`${inlineField} h-7 text-md font-semibold`}
                value={definition()?.name ?? ""}
                placeholder="Name the dashboard"
                disabled={!definition()}
                onInput={(e) => {
                  const name = e.currentTarget.value;
                  changeDefinition((current) => ({ ...current, name }));
                }}
              />
              <input
                aria-label="Description"
                class={`${inlineField} h-6 text-muted`}
                value={definition()?.description ?? ""}
                placeholder="Add a description"
                disabled={!definition()}
                onInput={(e) => {
                  const description = e.currentTarget.value;
                  changeDefinition((current) => ({ ...current, description }));
                }}
              />
            </div>
            <div class="flex items-center gap-2">
              <Button
                class="flex items-center gap-1.5"
                disabled={!definition()}
                onClick={() => setOpenEditor({ kind: "new" })}
              >
                <PlusIcon size={13} />
                Widget
              </Button>
              <Show when={hasChanges()}>
                <Button disabled={pendingAction() !== undefined} onClick={discardChanges}>
                  Discard
                </Button>
                <Button
                  variant="primary"
                  title="Save (⌘S)"
                  disabled={pendingAction() !== undefined}
                  onClick={() => void saveChanges()}
                >
                  {pendingAction() === "save" ? "Saving…" : "Save"}
                </Button>
              </Show>
              <Show when={definition()}>
                <ActionMenu label="Dashboard actions" items={dashboardMenuItems} />
              </Show>
            </div>
          </div>
        }
        end={
          <>
            <RangePicker
              since={latest(range.since)}
              until={latest(range.until)}
              onChange={(since, until) => range.setRange(since, until)}
            />
            <LiveToggle
              live={latest(range.live)}
              until={latest(range.until)}
              onChange={(live) => range.setLive(live)}
            />
          </>
        }
      >
        <a href={`/dashboards${range.toSearch()}`} class={link}>
          Dashboards
        </a>
        <Show when={pendingAction() === "clone"}>
          <span aria-live="polite">Cloning…</span>
        </Show>
        <Show when={hasChanges() && pendingAction() !== "clone"}>
          <span aria-live="polite">Unsaved changes</span>
        </Show>
      </PageBar>

      <div class={`min-h-0 flex-1 ${pageContent}`}>
        <div>
          <Show when={actionError() ?? fetched.errorMessage()}>
            {(errorMessage) => <Callout tone="error">{errorMessage()}</Callout>}
          </Show>
        </div>
        <FetchErrorBoundary>
          <Show when={definition()}>
            {(shown) => (
              <Show
                when={shown().widgets.length > 0}
                fallback={
                  <div class="flex flex-col items-center gap-3 py-16 text-center">
                    <p class="m-0 text-muted">This dashboard has no widgets yet.</p>
                    <Button
                      variant="primary"
                      class="flex items-center gap-1.5"
                      onClick={() => setOpenEditor({ kind: "new" })}
                    >
                      <PlusIcon size={13} />
                      Add a widget
                    </Button>
                  </div>
                }
              >
                <DashboardPageGrid
                  widgets={shown().widgets}
                  range={range}
                  onEdit={(index) => {
                    const widget = shown().widgets[index];
                    if (widget) setOpenEditor({ kind: "existing", index, widget });
                  }}
                  onDuplicate={duplicateWidget}
                  onRemove={(index) => {
                    const widget = shown().widgets[index];
                    if (widget) setPendingRemoval({ index, title: describeWidgetTitle(widget) });
                  }}
                  onLayoutChange={(layouts) =>
                    changeWidgets((widgets) =>
                      widgets.map((widget, index) => ({
                        ...widget,
                        layout: layouts[index] ?? widget.layout,
                      })),
                    )
                  }
                />
              </Show>
            )}
          </Show>
        </FetchErrorBoundary>
      </div>

      <Show when={deleting()}>
        <ConfirmDialog
          title="Delete the dashboard?"
          message={`${definition()?.name ?? "The dashboard"} and its widgets go for good.`}
          confirmLabel="Delete"
          busy={pendingAction() === "delete"}
          onConfirm={() => void deleteThisDashboard()}
          onCancel={() => setDeleting(false)}
        />
      </Show>
      <Show when={pendingRemoval()}>
        {(removal) => (
          <ConfirmDialog
            title="Remove the widget?"
            message={`${removal().title} leaves the dashboard when you save it.`}
            confirmLabel="Remove"
            onConfirm={() => removeWidget(removal().index)}
            onCancel={() => setPendingRemoval(undefined)}
          />
        )}
      </Show>
      <Show when={pendingLeave()}>
        {(leave) => (
          <ConfirmDialog
            title="Leave without saving?"
            message="The changes to this dashboard go away. Save them first to keep them."
            confirmLabel="Leave"
            onConfirm={() => {
              setPendingLeave(undefined);
              leave().retry(true);
            }}
            onCancel={() => setPendingLeave(undefined)}
          />
        )}
      </Show>
      <Show when={openEditor()}>
        {(editor) => (
          <WidgetEditor
            heading={editor().kind === "new" ? "Add a widget" : "Edit the widget"}
            widget={pickEditedWidget(editor())}
            range={range}
            onApply={(widget) => applyEditedWidget(editor(), widget)}
            onClose={() => setOpenEditor(undefined)}
          />
        )}
      </Show>
    </div>
  );
}

function settleWidgets(widgets: ReadonlyArray<Widget>, pinnedIndex: number | undefined): Widget[] {
  const layouts = settleLayouts(
    widgets.map((widget) => widget.layout),
    pinnedIndex,
  );
  return widgets.map((widget, index) => ({ ...widget, layout: layouts[index] ?? widget.layout }));
}

function pickEditedWidget(editor: OpenEditor): Widget {
  return editor.kind === "existing" ? editor.widget : createDefaultWidget();
}

function warnAboutUnsavedChanges(e: BeforeUnloadEvent) {
  e.preventDefault();
}
