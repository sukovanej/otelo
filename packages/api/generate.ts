// Writes src/schema.ts, the TypeScript types of openapi.json, which
// `siner openapi` prints from the Rust types of the daemon. `mise run
// api:generate` runs both, and a test of each side fails when its file is
// behind.

import { readFileSync, writeFileSync } from "node:fs";

import openapiTS, { astToString } from "openapi-typescript";

const HEADER = `// Generated from openapi.json by generate.ts; \`mise run api:generate\`
// writes it again. Do not edit it by hand.

`;

const SPEC = new URL("openapi.json", import.meta.url);
const SCHEMA = new URL("src/schema.ts", import.meta.url);

/** The text of src/schema.ts for the spec in openapi.json. */
export async function generate(): Promise<string> {
  const spec: unknown = JSON.parse(readFileSync(SPEC, "utf8"));
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- openapi-typescript checks the spec
  const ast = await openapiTS(spec as Parameters<typeof openapiTS>[0]);
  return HEADER + astToString(ast);
}

/** src/schema.ts as it is on the disk. */
export const written = () => readFileSync(SCHEMA, "utf8");

if (import.meta.main) {
  writeFileSync(SCHEMA, await generate());
}
