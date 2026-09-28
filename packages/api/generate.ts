// Writes src/schema.ts, the TypeScript types of openapi.json, which
// `siner openapi` prints from the Rust types of the daemon. `mise run
// api:generate` runs both, and a test of each side fails when its file is
// behind.

import { readFileSync, writeFileSync } from "node:fs";

import openapiTS, { astToString, type SchemaObject } from "openapi-typescript";
import ts from "typescript";

const HEADER = `// Generated from openapi.json by generate.ts; \`mise run api:generate\`
// writes it again. Do not edit it by hand.

`;

const SPEC = new URL("openapi.json", import.meta.url);
const SCHEMA = new URL("src/schema.ts", import.meta.url);

/** The text of src/schema.ts for the spec in openapi.json. */
export async function generate(): Promise<string> {
  const spec: unknown = JSON.parse(readFileSync(SPEC, "utf8"));
  const recursive = new Set<string>();
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- openapi-typescript checks the spec
  const ast = await openapiTS(spec as Parameters<typeof openapiTS>[0], {
    transform: (schema, { path }) => {
      const name = selfArray(schema, path);
      if (name === undefined) return undefined;
      recursive.add(name);
      return ts.factory.createTypeReferenceNode(arrayOf(name));
    },
  });
  const aliases = [...recursive].map(
    (name) => `type ${arrayOf(name)} = components["schemas"]["${name}"][];\n`,
  );
  return HEADER + astToString(ast) + aliases.join("");
}

// A schema that holds an array of itself, such as AttributeValue, names the
// array with a type alias. TypeScript resolves an array type inside an
// interface at once, so `components["schemas"]["AttributeValue"][]` inside
// AttributeValue refers to itself; inside an alias it waits until it is used.

/** The name of the schema at `path` when `schema` is an array of it. */
function selfArray(schema: SchemaObject, path: string | undefined): string | undefined {
  if (schema.type !== "array" || !schema.items || !("$ref" in schema.items)) return undefined;
  const ref = schema.items.$ref;
  if (path !== ref && !path?.startsWith(`${ref}/`)) return undefined;
  return ref.slice(ref.lastIndexOf("/") + 1);
}

const arrayOf = (name: string) => `ArrayOf${name}`;

/** src/schema.ts as it is on the disk. */
export const written = () => readFileSync(SCHEMA, "utf8");

if (import.meta.main) {
  writeFileSync(SCHEMA, await generate());
}
