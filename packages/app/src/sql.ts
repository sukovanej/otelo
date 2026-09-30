const SQL_KEYWORDS = new Set(
  `ADD ALL ALTER ANALYZE AND ANY AS ASC BEGIN BETWEEN BY CALL CASE CAST CHECK COLLATE COLUMN
  COMMIT CONFLICT CONSTRAINT CREATE CROSS DATABASE DECLARE DEFAULT DELETE DESC DISTINCT DO DROP
  DUPLICATE ELSE END ESCAPE EXCEPT EXEC EXECUTE EXISTS EXPLAIN FALSE FETCH FILTER FIRST FOR
  FOREIGN FROM FULL GROUP HAVING IF IGNORE ILIKE IN INDEX INNER INSERT INTERSECT INTO IS JOIN KEY
  LAST LATERAL LEFT LIKE LIMIT LOCK LOCKED MATCHED MERGE NATURAL NEXT NO NOT NOTHING NOWAIT NULL
  NULLS OFFSET ON ONLY OR ORDER OUTER OVER PARTITION PRAGMA PRIMARY QUALIFY RECURSIVE REFERENCES
  RELEASE RENAME REPLACE RETURNING RIGHT ROLLBACK ROW ROWS SAVEPOINT SCHEMA SELECT SET SHARE SHOW
  SKIP SOME START TABLE TEMP TEMPORARY THEN TO TOP TRANSACTION TRUE TRUNCATE UNION UNIQUE UPDATE
  USE USING VACUUM VALUES VIEW WHEN WHERE WINDOW WITH`.split(/\s+/),
);

// The first rule that matches at a place makes the token there. The keyword
// rule takes every word, and `lexSql` keeps it a keyword only when
// `SQL_KEYWORDS` has it.
const TOKEN_RULES: ReadonlyArray<SqlRule> = [
  ["space", /\s+/],
  ["comment", /--[^\n]*|\/\*[\s\S]*?(?:\*\/|$)/],
  ["string", /'(?:[^']|'')*(?:'|$)/],
  ["text", /"(?:[^"]|"")*(?:"|$)|`[^`]*(?:`|$)/],
  ["keyword", /[\p{L}_][\p{L}\p{N}_$]*/u],
  ["number", /0[xX][0-9a-fA-F]+|\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/],
  // A cast, such as `x::text`, which is not a parameter named `:text`.
  ["text", /::/],
  ["placeholder", /\?|\$\d+|[:@][A-Za-z_]\w*|%s\b|%\([A-Za-z_]\w*\)s/],
  ["punctuation", /[(),;.]/],
  ["text", /[\s\S]/],
];

const TOKEN_PATTERN = new RegExp(
  TOKEN_RULES.map(([, pattern]) => `(${pattern.source})`).join("|"),
  "gu",
);

const SPACE_TOKEN: SqlToken = { kind: "space", text: " " };

const CLAUSE_KEYWORDS = new Set(
  "SELECT WHERE HAVING LIMIT OFFSET RETURNING WINDOW QUALIFY UNION INTERSECT EXCEPT SET".split(" "),
);

const JOIN_MODIFIERS = new Set("INNER LEFT RIGHT FULL OUTER CROSS NATURAL".split(" "));

const SUBQUERY_FIRST_KEYWORDS = new Set("SELECT WITH VALUES INSERT UPDATE DELETE".split(" "));

export type SqlTokenKind =
  | "keyword"
  | "string"
  | "number"
  | "placeholder"
  | "comment"
  | "punctuation"
  | "space"
  | "text";

export interface SqlToken {
  readonly kind: SqlTokenKind;
  readonly text: string;
}

interface SqlLine {
  readonly indentLevel: number;
  readonly tokens: SqlToken[];
}

type SqlRule = readonly [kind: SqlTokenKind, pattern: RegExp];

interface LayoutToken {
  readonly token: SqlToken;
  readonly normalizedText: string;
  readonly followsSpace: boolean;
}

export function lexSql(text: string): SqlToken[] {
  const tokens: SqlToken[] = [];
  for (const match of text.matchAll(TOKEN_PATTERN)) {
    // The group that took part is the rule that matched: no rule matches
    // nothing, so the group of each other rule is not a string.
    const rule = TOKEN_RULES[match.slice(1).findIndex(Boolean)];
    const kind = rule?.[0] ?? "text";
    tokens.push({
      kind: kind === "keyword" && !SQL_KEYWORDS.has(match[0].toUpperCase()) ? "text" : kind,
      text: match[0],
    });
  }
  return tokens;
}

export function layOutSql(text: string): SqlLine[] {
  const layoutTokens: LayoutToken[] = [];
  let sawSpace = false;
  for (const token of lexSql(text)) {
    if (token.kind === "space") {
      sawSpace = true;
      continue;
    }
    const nonKeywordText = token.kind === "string" || token.kind === "comment" ? "" : token.text;
    layoutTokens.push({
      token,
      normalizedText: token.kind === "keyword" ? token.text.toUpperCase() : nonKeywordText,
      followsSpace: sawSpace,
    });
    sawSpace = false;
  }

  const lines: SqlLine[] = [];
  let line: SqlLine = { indentLevel: 0, tokens: [] };
  const startLine = (indentLevel: number) => {
    if (line.tokens.length > 0) lines.push(line);
    line = { indentLevel, tokens: [] };
  };
  const openParenSubqueryIndents: (number | undefined)[] = [];
  let clauseIndent = 0;
  let openCaseCount = 0;
  let betweenAwaitsAnd = false;
  let nextStartsLine = false;

  layoutTokens.forEach(({ token, normalizedText, followsSpace }, index) => {
    let lineIndent: number | undefined;
    if (normalizedText === ")") {
      const closedSubqueryIndent = openParenSubqueryIndents.pop();
      if (closedSubqueryIndent !== undefined) {
        clauseIndent =
          openParenSubqueryIndents.findLast((subqueryIndent) => subqueryIndent !== undefined) ?? 0;
        lineIndent = closedSubqueryIndent - 1;
      }
    }
    const inArguments =
      openParenSubqueryIndents.length > 0 &&
      openParenSubqueryIndents[openParenSubqueryIndents.length - 1] === undefined;
    if (lineIndent === undefined && nextStartsLine) {
      lineIndent = inArguments ? line.indentLevel : clauseIndent;
    }
    if (lineIndent === undefined && !inArguments && normalizedText !== ")") {
      if (startsClause(layoutTokens, index)) {
        lineIndent = clauseIndent;
      } else if (
        openCaseCount === 0 &&
        (normalizedText === "OR" || (normalizedText === "AND" && !betweenAwaitsAnd))
      ) {
        lineIndent = clauseIndent + 1;
      }
    }

    if (normalizedText === "AND") betweenAwaitsAnd = false;
    else if (normalizedText === "BETWEEN") betweenAwaitsAnd = true;
    else if (normalizedText === "CASE") openCaseCount++;
    else if (normalizedText === "END" && openCaseCount > 0) openCaseCount--;

    if (lineIndent !== undefined) {
      startLine(lineIndent);
    } else if (
      line.tokens.length > 0 &&
      (followsSpace || layoutTokens[index - 1]?.normalizedText === ",")
    ) {
      line.tokens.push(SPACE_TOKEN);
    }
    line.tokens.push(token);

    nextStartsLine =
      normalizedText === ";" || (token.kind === "comment" && token.text.startsWith("--"));
    if (normalizedText === "(") {
      const subqueryIndent = SUBQUERY_FIRST_KEYWORDS.has(
        layoutTokens[index + 1]?.normalizedText ?? "",
      )
        ? line.indentLevel + 1
        : undefined;
      openParenSubqueryIndents.push(subqueryIndent);
      if (subqueryIndent !== undefined) {
        clauseIndent = subqueryIndent;
        nextStartsLine = true;
      }
    }
  });
  startLine(0);
  return lines;
}

function startsClause(layoutTokens: ReadonlyArray<LayoutToken>, index: number): boolean {
  const current = layoutTokens[index]?.normalizedText ?? "";
  const previous = layoutTokens[index - 1]?.normalizedText ?? "";
  const next = layoutTokens[index + 1]?.normalizedText ?? "";
  if (CLAUSE_KEYWORDS.has(current)) return true;
  switch (current) {
    case "FROM":
      return previous !== "DELETE" && previous !== "DISTINCT";
    case "VALUES":
      return previous !== "=" && previous !== "DEFAULT";
    case "GROUP":
    case "ORDER":
      return next === "BY";
    case "INSERT":
    case "UPDATE":
    case "DELETE":
      return previous === ")";
    case "ON":
      return next === "CONFLICT" || next === "DUPLICATE";
    case "FOR":
      return next === "UPDATE" || next === "SHARE" || next === "NO" || next === "KEY";
    case "FETCH":
      return next === "FIRST" || next === "NEXT";
    case "JOIN":
      return !JOIN_MODIFIERS.has(previous);
    default: {
      if (!JOIN_MODIFIERS.has(current) || JOIN_MODIFIERS.has(previous)) return false;
      let joinIndex = index + 1;
      while (JOIN_MODIFIERS.has(layoutTokens[joinIndex]?.normalizedText ?? "")) joinIndex++;
      return layoutTokens[joinIndex]?.normalizedText === "JOIN";
    }
  }
}
