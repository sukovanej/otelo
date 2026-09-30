---
name: frontend-typescript
description: How otelo writes frontend TypeScript. Where a component lives, what its file is called, what it exports, the order inside the file, how its types are shaped, and how everything is named. Invoke before creating or editing any .ts or .tsx file under packages/, and before reviewing frontend code.
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
