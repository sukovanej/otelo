import type { Attributes, AttributeValue } from "@otelo/api";

import { quoteString, writeAttributeField, writeLiteral, writeResourceField } from "./query";
import { isSqlQueryAttribute } from "./semantics";

export interface Field {
  readonly label: string;
  readonly value: AttributeValue;
  readonly query: FieldQuery;
  readonly isSqlQuery: boolean;
}

export interface FieldSection {
  readonly title: string;
  readonly fields: ReadonlyArray<Field>;
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
  return { label: name, value, query: toFieldQuery(name, literal), isSqlQuery: false };
}

export function toUnnamedField(label: string, value: AttributeValue): Field {
  return { label, value, query: { kind: "unnamed" }, isSqlQuery: false };
}

export function listAttributeFields(attributes: Attributes): Field[] {
  return sortByKey(attributes).map(([key, value]) => ({
    label: key,
    value,
    query: toFieldQuery(writeAttributeField(key), writeLiteral(value)),
    isSqlQuery: isSqlQueryAttribute(attributes, key),
  }));
}

export function listResourceFields(resource: Attributes): Field[] {
  return sortByKey(resource).map(([key, value]) => ({
    label: key,
    value,
    query: toFieldQuery(writeResourceField(key), writeLiteral(value)),
    isSqlQuery: false,
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
