---
status: todo
created: 2026-10-05T14:40:03Z
tags:
- bug
---
# Fix the Solid warnings on the dashboard page

Under `mise run web:dev`, a dashboard page logs two kinds of Solid warning in the browser console.

- `STRICT_READ_UNTRACKED`: "Reactive value "condition" read directly in <Match> will not update", and the same for "condition value". It comes from the `<Switch>` in `DashboardWidgetData`, on the path `<DashboardWidget> › <Show> › <DashboardWidgetData> › <Errored> › <Switch>`. A widget can then keep drawing the branch it first picked.
- `EFFECT_RELAY_TEAR`: an effect in `DashboardPageView` writes its compute output into `definition` on every run, and another one into `savedDefinition`. Each is derived state one flush late. Make it a memo, or a writable derived signal, `createSignal(() => source)`, where the page edits it.

`packages/app/node_modules/solid-js/skills/reactivity-diagnostics/SKILL.md` explains both.

Done when a dashboard page logs neither warning, and editing, saving, and discarding a dashboard still work.
