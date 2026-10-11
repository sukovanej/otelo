import type { Attributes, AttributeValue } from "@otelo/api";
import type { LevelName } from "@otelo/ui";

import { quoteString, writeAttributeField, writeLiteral, writeResourceField } from "./query";
import { isSqlQueryAttribute } from "./semantics";

export interface Field {
  readonly label: string;
  readonly value: AttributeValue;
  readonly query: FieldQuery;
  readonly view: FieldView;
  readonly href: string | null;
}

export interface FieldSection {
  readonly title: string;
  readonly fields: ReadonlyArray<Field>;
}

export type FieldView =
  | TextFieldView
  | SqlQueryFieldView
  | LevelFieldView
  | SpanKindFieldView
  | SpanStatusFieldView;

interface TextFieldView {
  readonly kind: "text";
}

interface SqlQueryFieldView {
  readonly kind: "sql_query";
}

interface LevelFieldView {
  readonly kind: "level";
  readonly level: LevelName;
}

interface SpanKindFieldView {
  readonly kind: "span_kind";
}

interface SpanStatusFieldView {
  readonly kind: "span_status";
}

type FieldQuery = ComparableField | NamedField | UnnamedField;

interface ComparableField {
  readonly kind: "comparable";
  readonly name: string;
  readonly literal: string;
}

interface NamedField {
  readonly kind: "named";
  readonly name: string;
}

interface UnnamedField {
  readonly kind: "unnamed";
}

export function toBuiltinField(
  name: string,
  value: string | null,
  literal = value && quoteString(value),
): Field {
  return {
    label: name,
    value,
    query: toFieldQuery(name, literal),
    view: { kind: "text" },
    href: null,
  };
}

export function toUnnamedField(label: string, value: AttributeValue): Field {
  return { label, value, query: { kind: "unnamed" }, view: { kind: "text" }, href: null };
}

export function linkField(field: Field, href: string | null): Field {
  return { ...field, href };
}

export function viewField(field: Field, view: FieldView): Field {
  return { ...field, view };
}

export function listAttributeFields(attributes: Attributes): Field[] {
  return sortByKey(attributes).map(([key, value]) => ({
    label: key,
    value,
    query: toFieldQuery(writeAttributeField(key), writeLiteral(value)),
    view: { kind: isSqlQueryAttribute(attributes, key) ? "sql_query" : "text" },
    href: null,
  }));
}

export function listResourceFields(resource: Attributes): Field[] {
  return sortByKey(resource).map(([key, value]) => ({
    label: key,
    value,
    query: toFieldQuery(writeResourceField(key), writeLiteral(value)),
    view: { kind: "text" },
    href: null,
  }));
}

function toFieldQuery(name: string | undefined, literal: string | null | undefined): FieldQuery {
  if (name === undefined) return { kind: "unnamed" };
  return literal ? { kind: "comparable", name, literal } : { kind: "named", name };
}

type AttributeEntry = readonly [key: string, value: AttributeValue];

function sortByKey(attributes: Attributes): AttributeEntry[] {
  return Object.entries(attributes).toSorted(([key], [otherKey]) => key.localeCompare(otherKey));
}
