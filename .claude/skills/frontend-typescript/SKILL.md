---
name: frontend-typescript
description: How otelo writes frontend TypeScript on Solid 2. Where a component lives, what its file is called, what it exports, the order inside the file, how its types are shaped, how everything is named, and where Solid 2 differs from Solid 1. Invoke before creating or editing any .ts or .tsx file under packages/, and before reviewing frontend code.
---

# Writing the frontend TypeScript

Rules for every `.ts` and `.tsx` in `packages/`. Copy `packages/ui/src/QueryInput/`.

## Generated files

Never write `packages/api/openapi.json` or `packages/api/src/schema.ts` by hand, not one
line of them. They come only from `mise run api:generate`. To change one, change the Rust
handler or its types and regenerate. Keep everything the generator writes, even lines your
change did not touch.

## Where it lives

- Pure components in `packages/ui`, the ones that show data in `packages/viz`, icons in
  `packages/icons`. None of them imports `@otelo/api`. The app wraps them with data.
- A component that reaches the daemon, or lays out a whole page, stays in `packages/app`.

## File names

- One component per file.
- Public component: CamelCase, `SidePanel.tsx`.
- Private component: kebab-case, `query-input-suggestions.tsx`.
- Several sub-components: a directory, public one in `index.tsx`, private ones beside it.

## Exports

Never export what only its own file or directory uses, props interfaces included.

## Order in a file

Imports, constants, types, components, helpers.

- Exported before private.
- Props interface directly above the component that takes them.
- A helper's own types directly above that helper.

## Types

- `interface` over `type`. `type` only for a union or a non-object alias.
- Named types only. No anonymous object on a field, no intersection around an inline union.
- Make invalid states unrepresentable. Three states are three shapes, not four optional
  fields.
- An optional field must earn it, and the code says what its absence means.

### Unions

```ts
type Following = FollowingToken | FollowingWhitespace | FollowingNothing;

interface FollowingToken {
  readonly kind: "token";
  readonly tokenType: TokenType;
  readonly word: string;
}
```

- Named interfaces extending a shared base where there is one. Union alias above them.
- The discriminator is named for what it tells apart: `kind`, `state`, `view`, `variant`.
  A component's props union takes one the same way.
- Lower-case tags, as the daemon writes its own: `"attribute"`, `"builtin"`.
- A discriminator off the wire keeps its name and its tags. `FieldSource` is the daemon's
  shape, not yours.
- Branch with `following.kind === "token"`, or with a `switch` of statements.
- Map a discriminator to a value with a `Record<Kind, …>`, where a missing tag does not
  compile. A `switch` that returns from every case fails the `consistent-return` lint.

## Solid 2

The UI runs on Solid 2, a release candidate newer than your training, so code written from
memory comes out as Solid 1. Look an API up at https://v2.solidjs.com before you use it.

- **Writes are staged.** A setter commits on the next microtask. A read right after it
  returns the old value, and the DOM still shows the old state. A handler that focuses,
  scrolls to or measures what it just changed calls `flush()` after the write, as
  `Select.openList` does.
- **Effects have two phases.** `createEffect(() => source(), (value) => { … })` reads in the
  first function and acts in the second, which returns its cleanup.
- **State that follows a source and can still be edited** is a writable derived signal,
  `createSignal(() => props.value)`. It resets when the source changes.
- **A prop read once on purpose** goes through `untrack(() => props.initialSpanId)`. Every
  other prop read sits in JSX, a memo or the first function of an effect.
- **Fetching goes through `createFetch`** in `fetch.ts`, which wraps `useQuery` of
  `@tanstack/solid-query`. Each caller names its query, so two fetches with the same key
  keep apart in the cache. Solid holds every reader of a changed key until the answer
  lands, which is right, so acknowledge the wait and never route around it. A control
  that echoes its own source reads it through `latest(range.since)`, or it lags a click
  and a second click builds on the old value. `loading()` shows the wait.
- **Every read of fetched data sits under an `<Errored>`**: `FetchErrorBoundary` around
  content, and an `<Errored fallback={…}>` drawing the same element without the data
  elsewhere. A request that fails for a new key with no boundary above leaves the hold
  open, and the page stops answering. A view keyed on what it fetches, as `TracePage`
  keys `TraceView` by the trace id, never changes key, so its reads need no boundary.
- **Cleanups only release.** `onCleanup` and the cleanup an effect returns clear timers,
  remove listeners and abort requests, and leave signals untouched. In rc.13 a signal
  write during disposal held every later update, and the router froze with it.

The typecheck rejects the Solid 1 form. This is the Solid 2 one:

| Solid 1                          | Solid 2                                               |
| -------------------------------- | ----------------------------------------------------- |
| `<Index>`                        | `<For keyed={false}>`                                 |
| `onMount`                        | `onSettled`                                           |
| `on(source, fn)`                 | the first function of `createEffect`                  |
| `batch`                          | nothing, since writes batch already                   |
| `splitProps`, `mergeProps`       | `omit`, `merge`                                       |
| `classList`                      | `class={["base", { "opacity-60": loading() }]}`       |
| `<Dynamic component={…}>`        | `const PickedIcon = dynamic(() => …)`                 |
| `aria-expanded={open()}`         | `aria-expanded={open() ? "true" : "false"}`           |
| `tabIndex`, `on:keydown`         | `tabindex`, `onKeyDown`                               |
| `<A activeClass>`                | `<a>` styled with `data-active:`                      |
| `solid-js/web`, `JSX`            | `@solidjs/web`                                        |

A boolean `aria-*` value renders as an empty attribute, which `aria-selected:` and the
other Tailwind variants do not match.

A change is done when `mise check` passes and the pages it touches, served by
`mise run web:dev`, add no new warning to the browser console. The dev build names
reactivity mistakes there, and
`packages/app/node_modules/solid-js/skills/reactivity-diagnostics/SKILL.md` explains each one.

## Names

- A name says what the thing is, in full. A lone verb almost never does.
- Functions take a verb *and* what it acts on: `lexQuery`, `splitIntoPieces`.
- The verb is the domain action, never the machinery it is built from.
- A qualifier is not a description: `applyOnce` says how many, not what.
- Components are named for what they draw.
- Constants for what they hold: `TOKEN_CLASSES`, not `TOKENS`.

## No comments

- None. A comment means the code is not self-documenting. Rename or reshape until it is.
- The only one that earns its place is intent the code cannot express. Nothing else does.
- Delete the ones you meet that do not.
