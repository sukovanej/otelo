// Says what each token of a query is, for the colors of the query input. It
// follows `highlight_tokens` in crates/query/src/highlight.rs, and a test
// holds it to the snapshots of that one.

import type { QueryToken, TokenKind } from "@otelo/ui";

import { isKeyword, lex, type Token, type TokenType } from "./lexer";

/** What the tokens before say of the next one: it starts a term, it follows
 * a field, it follows `in` or `has`, it is the field of `has(`, or it is a
 * value. */
type Position = "term" | "afterField" | "afterIn" | "afterHas" | "hasField" | "value";

/** What follows a word: a token, with its text when it is a word, only
 * whitespace, or nothing. */
type Following = { type: TokenType; word: string } | "whitespace" | undefined;

const endsAFieldName = (following: Following) =>
  following === "whitespace" ||
  following?.type === "operator" ||
  (following?.type === "word" && following.word.toLowerCase() === "in");

function highlightWord(
  word: string,
  position: Position,
  following: Following,
): [TokenKind, Position] {
  if (isKeyword(word)) return ["keyword", "term"];
  if (position === "value") {
    return [word === "true" || word === "false" ? "boolean" : "string", "term"];
  }
  const lower = word.toLowerCase();
  if (position === "afterField" && lower === "in") return ["keyword", "afterIn"];
  if (position === "hasField") return ["field", "afterField"];
  if (lower === "has" && typeof following === "object" && following.type === "(") {
    return ["keyword", "afterHas"];
  }
  // A word still being typed may become a keyword, such as the `an` of `and`.
  return endsAFieldName(following) ? ["field", "afterField"] : ["word", "term"];
}

/** The kind of each token that is not a word, and the position after it. */
const SYMBOLS: Record<Exclude<TokenType, "word">, [TokenKind, Position]> = {
  backticked: ["field", "afterField"],
  string: ["string", "term"],
  unclosed: ["invalid", "term"],
  unreadable: ["invalid", "term"],
  number: ["number", "term"],
  operator: ["operator", "value"],
  ",": ["punctuation", "value"],
  "(": ["punctuation", "term"],
  ")": ["punctuation", "term"],
};

/** The position after the parenthesis of `in (`, and of `has(`. */
const OPENS: Partial<Record<Position, Position>> = { afterIn: "value", afterHas: "hasField" };

export function highlight(query: string): QueryToken[] {
  const tokens = lex(query);
  let position: Position = "term";
  const text = (token: Token) => query.slice(token.start, token.end);
  return tokens.map((token, i) => {
    const after = tokens[i + 1];
    const following: Following = after
      ? { type: after.type, word: text(after) }
      : token.end < query.length
        ? "whitespace"
        : undefined;
    const [kind, positionAfter] =
      token.type === "word" ? highlightWord(text(token), position, following) : SYMBOLS[token.type];
    position = (token.type === "(" ? OPENS[position] : undefined) ?? positionAfter;
    return { start: token.start, end: token.end, kind };
  });
}
