// SQL as the UI shows it: split into tokens, for their colors, and laid out
// over lines, a clause on each. It reads every dialect loosely and never
// fails: what it does not know is plain text.

export type SqlKind =
  | "keyword"
  | "string"
  | "number"
  | "placeholder"
  | "comment"
  | "punctuation"
  | "space"
  | "text";

export interface SqlToken {
  kind: SqlKind;
  text: string;
}

const KEYWORDS = new Set(
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

// The first rule that matches at a place makes the token there. A string
// or a comment without its end runs to the end of the text. A quoted name
// is text, as a bare one is, and a word is a keyword when `KEYWORDS` has it.
const RULES: [SqlKind, RegExp][] = [
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

const TOKEN = new RegExp(RULES.map(([, rule]) => `(${rule.source})`).join("|"), "gu");

/** The tokens of `text`, which join back into it. */
export function sqlTokens(text: string): SqlToken[] {
  const tokens: SqlToken[] = [];
  for (const match of text.matchAll(TOKEN)) {
    // The group that took part is the rule that matched: no rule matches
    // nothing, so the group of each other rule is not a string.
    const rule = RULES[match.slice(1).findIndex(Boolean)];
    const kind = rule?.[0] ?? "text";
    tokens.push({
      kind: kind === "keyword" && !KEYWORDS.has(match[0].toUpperCase()) ? "text" : kind,
      text: match[0],
    });
  }
  return tokens;
}

/** One line of a query laid out: its tokens, without the space that leads
 * it, and how many levels deep it is. */
export interface SqlLine {
  indent: number;
  tokens: SqlToken[];
}

/** A token that is not space, as the layout reads it. */
interface Word {
  token: SqlToken;
  /** The keyword in upper case, or the text of any other token that is not
   * a string or a comment. */
  word: string;
  /** Whether space comes before it in the text. */
  gap: boolean;
}

const SPACE: SqlToken = { kind: "space", text: " " };

/** The keywords that start a clause wherever they are. */
const CLAUSES = new Set(
  "SELECT WHERE HAVING LIMIT OFFSET RETURNING WINDOW QUALIFY UNION INTERSECT EXCEPT SET".split(" "),
);

/** The keywords that come before `JOIN` in the name of a join. */
const JOINS = new Set("INNER LEFT RIGHT FULL OUTER CROSS NATURAL".split(" "));

/** What a query in parentheses starts with. */
const QUERIES = new Set("SELECT WITH VALUES INSERT UPDATE DELETE".split(" "));

/** Whether the word at `i` starts a clause, which starts a line. */
function startsClause(words: Word[], i: number): boolean {
  const word = words[i]?.word ?? "";
  const prev = words[i - 1]?.word ?? "";
  const next = words[i + 1]?.word ?? "";
  if (CLAUSES.has(word)) return true;
  switch (word) {
    case "FROM":
      return prev !== "DELETE" && prev !== "DISTINCT";
    case "VALUES":
      return prev !== "=" && prev !== "DEFAULT";
    case "GROUP":
    case "ORDER":
      return next === "BY";
    case "INSERT":
    case "UPDATE":
    case "DELETE":
      return prev === ")";
    case "ON":
      return next === "CONFLICT" || next === "DUPLICATE";
    case "FOR":
      return next === "UPDATE" || next === "SHARE" || next === "NO" || next === "KEY";
    case "FETCH":
      return next === "FIRST" || next === "NEXT";
    case "JOIN":
      return !JOINS.has(prev);
    default: {
      if (!JOINS.has(word) || JOINS.has(prev)) return false;
      let join = i + 1;
      while (JOINS.has(words[join]?.word ?? "")) join++;
      return words[join]?.word === "JOIN";
    }
  }
}

/**
 * A query laid out over lines, whatever lines and spaces it came with: each
 * clause starts a line, each `AND` and `OR` between conditions starts one a
 * level deeper, and a query in parentheses is a level deeper than the line
 * that opens it. What other parentheses hold, such as the arguments of a
 * function, stays on its line. The tokens keep their text and their case.
 */
export function sqlLines(text: string): SqlLine[] {
  const words: Word[] = [];
  let spaced = false;
  for (const token of sqlTokens(text)) {
    if (token.kind === "space") {
      spaced = true;
      continue;
    }
    const plain = token.kind === "string" || token.kind === "comment" ? "" : token.text;
    words.push({
      token,
      word: token.kind === "keyword" ? token.text.toUpperCase() : plain,
      gap: spaced,
    });
    spaced = false;
  }

  const lines: SqlLine[] = [];
  let line: SqlLine = { indent: 0, tokens: [] };
  const startLine = (indent: number) => {
    if (line.tokens.length > 0) lines.push(line);
    line = { indent, tokens: [] };
  };
  // The indent of the query each open parenthesis holds, or `undefined` for
  // one that holds no query.
  const parens: (number | undefined)[] = [];
  // The indent of a clause here.
  let depth = 0;
  // How many `CASE` are open, whose `AND` and `OR` stay on their line.
  let cases = 0;
  // Whether a `BETWEEN` waits for its `AND`.
  let between = false;
  // Whether the next token starts a line whatever it is.
  let forced = false;

  words.forEach(({ token, word, gap }, i) => {
    let indent: number | undefined;
    if (word === ")") {
      const inner = parens.pop();
      if (inner !== undefined) {
        depth = parens.findLast((held) => held !== undefined) ?? 0;
        indent = inner - 1;
      }
    }
    const inArguments = parens.length > 0 && parens[parens.length - 1] === undefined;
    if (indent === undefined && forced) indent = inArguments ? line.indent : depth;
    if (indent === undefined && !inArguments && word !== ")") {
      if (startsClause(words, i)) indent = depth;
      else if (cases === 0 && (word === "OR" || (word === "AND" && !between))) indent = depth + 1;
    }

    if (word === "AND") between = false;
    else if (word === "BETWEEN") between = true;
    else if (word === "CASE") cases++;
    else if (word === "END" && cases > 0) cases--;

    if (indent !== undefined) startLine(indent);
    else if (line.tokens.length > 0 && (gap || words[i - 1]?.word === ",")) line.tokens.push(SPACE);
    line.tokens.push(token);

    forced = word === ";" || (token.kind === "comment" && token.text.startsWith("--"));
    if (word === "(") {
      const inner = QUERIES.has(words[i + 1]?.word ?? "") ? line.indent + 1 : undefined;
      parens.push(inner);
      if (inner !== undefined) {
        depth = inner;
        forced = true;
      }
    }
  });
  startLine(0);
  return lines;
}
