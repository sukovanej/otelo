import { readFileSync, writeFileSync } from "node:fs";

import openapiTS, { astToString, type SchemaObject } from "openapi-typescript";
import ts from "typescript";

const SCHEMA_HEADER = `// Generated from openapi.json by generate.ts; \`mise run api:generate\`
// writes it again. Do not edit it by hand.

`;

const SPEC_URL = new URL("openapi.json", import.meta.url);
const SCHEMA_URL = new URL("src/schema.ts", import.meta.url);

export async function generateSchema(): Promise<string> {
  const spec: unknown = JSON.parse(readFileSync(SPEC_URL, "utf8"));
  const selfArraySchemas = new Set<string>();
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- openapi-typescript checks the spec
  const ast = await openapiTS(spec as Parameters<typeof openapiTS>[0], {
    // A schema that holds an array of itself, such as AttributeValue, names
    // the array with a type alias. TypeScript resolves an array type inside
    // an interface at once, so `components["schemas"]["AttributeValue"][]`
    // inside AttributeValue refers to itself; inside an alias it waits until
    // it is used.
    transform: (schema, { path }) => {
      const schemaName = findSelfArraySchema(schema, path);
      if (schemaName === undefined) return undefined;
      selfArraySchemas.add(schemaName);
      return ts.factory.createTypeReferenceNode(toArrayAliasName(schemaName));
    },
  });
  const aliases = [...selfArraySchemas].map(
    (schemaName) =>
      `type ${toArrayAliasName(schemaName)} = components["schemas"]["${schemaName}"][];\n`,
  );
  return SCHEMA_HEADER + astToString(ast) + aliases.join("");
}

export function readWrittenSchema(): string {
  return readFileSync(SCHEMA_URL, "utf8");
}

function findSelfArraySchema(schema: SchemaObject, path: string | undefined): string | undefined {
  if (schema.type !== "array" || !schema.items || !("$ref" in schema.items)) return undefined;
  const itemsRef = schema.items.$ref;
  if (path !== itemsRef && !path?.startsWith(`${itemsRef}/`)) return undefined;
  return itemsRef.slice(itemsRef.lastIndexOf("/") + 1);
}

function toArrayAliasName(schemaName: string): string {
  return `ArrayOf${schemaName}`;
}

if (import.meta.main) {
  writeFileSync(SCHEMA_URL, await generateSchema());
}
