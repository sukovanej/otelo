import type { QueryToken, TokenKind } from "@otelo/ui";

import { isKeyword, lexQuery, type Token, type TokenType } from "./lexer";

const SYMBOL_HIGHLIGHTS: Record<Exclude<TokenType, "word">, Highlight> = {
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

const POSITION_AFTER_OPENING: Partial<Record<Position, Position>> = {
  afterIn: "value",
  afterHas: "hasField",
};

type Position = "term" | "afterField" | "afterIn" | "afterHas" | "hasField" | "value";

type Highlight = readonly [kind: TokenKind, positionAfter: Position];

type Following = FollowingToken | FollowingWhitespace | FollowingNothing;

interface FollowingToken {
  readonly kind: "token";
  readonly tokenType: TokenType;
  readonly word: string;
}

interface FollowingWhitespace {
  readonly kind: "whitespace";
}

interface FollowingNothing {
  readonly kind: "nothing";
}

// A port of `highlight_tokens` in crates/query/src/highlight.rs, which a test
// holds to the snapshots of that one.
export function highlightQuery(query: string): QueryToken[] {
  const tokens = lexQuery(query);
  let position: Position = "term";
  return tokens.map((token, index) => {
    const [kind, positionAfter] =
      token.type === "word"
        ? highlightWord(readToken(query, token), position, findFollowing(query, tokens, index))
        : SYMBOL_HIGHLIGHTS[token.type];
    position = (token.type === "(" ? POSITION_AFTER_OPENING[position] : undefined) ?? positionAfter;
    return { start: token.start, end: token.end, kind };
  });
}

function readToken(query: string, token: Token): string {
  return query.slice(token.start, token.end);
}

function findFollowing(query: string, tokens: ReadonlyArray<Token>, index: number): Following {
  const next = tokens[index + 1];
  if (next) return { kind: "token", tokenType: next.type, word: readToken(query, next) };
  const last = tokens[index];
  return last && last.end < query.length ? { kind: "whitespace" } : { kind: "nothing" };
}

function highlightWord(word: string, position: Position, following: Following): Highlight {
  if (isKeyword(word)) return ["keyword", "term"];
  if (position === "value") {
    return [word === "true" || word === "false" ? "boolean" : "string", "term"];
  }
  const lowerCaseWord = word.toLowerCase();
  if (position === "afterField" && lowerCaseWord === "in") return ["keyword", "afterIn"];
  if (position === "hasField") return ["field", "afterField"];
  if (lowerCaseWord === "has" && following.kind === "token" && following.tokenType === "(") {
    return ["keyword", "afterHas"];
  }
  // A word still being typed may become a keyword, such as the `an` of `and`.
  return endsFieldName(following) ? ["field", "afterField"] : ["undecided", "term"];
}

function endsFieldName(following: Following): boolean {
  if (following.kind === "whitespace") return true;
  if (following.kind === "nothing") return false;
  return (
    following.tokenType === "operator" ||
    (following.tokenType === "word" && following.word.toLowerCase() === "in")
  );
}
