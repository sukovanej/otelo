// Cuts a query of `otelo-query` into tokens, as `lex_tokens` in
// crates/query/src/lexer.rs does. Positions are UTF-16 indexes.

export type TokenType =
  | "word"
  /** An integer, a float, or a duration such as `250ms`. */
  | "number"
  /** A closed string in single or double quotes. */
  | "string"
  /** A closed field name in backticks. */
  | "backticked"
  /** A string or a backticked name without its closing quote. */
  | "unclosed"
  | "("
  | ")"
  | ","
  | "operator"
  /** A character no token starts with, or an integer too large to read. */
  | "unreadable";

export interface Token {
  type: TokenType;
  start: number;
  end: number;
}

const WHITE_SPACE = /\p{White_Space}/u;
const WORD_CHAR = /[\p{Alphabetic}\p{N}_.\-/:@]/u;
const KEYWORDS = new Set(["and", "or", "not"]);
const DURATION_UNITS = new Set(["ns", "us", "µs", "ms", "s", "m", "h", "d"]);
const MAX_INT = 2n ** 63n - 1n;
const MIN_INT = -(2n ** 63n);

export const isWordChar = (c: string) => WORD_CHAR.test(c);

export const isKeyword = (word: string) => KEYWORDS.has(word.toLowerCase());

/** The character at the UTF-16 index `index` of `text`, whole when it takes
 * two code units, or `""` past the end. */
function charAt(text: string, index: number): string {
  const code = text.codePointAt(index);
  return code === undefined ? "" : String.fromCodePoint(code);
}

function classifyWord(word: string): TokenType {
  const digits = word.startsWith("-") ? word.slice(1) : word;
  if (!/^[0-9]/.test(digits)) return "word";
  if (/^[0-9]+$/.test(digits)) {
    const n = BigInt(word);
    return MIN_INT <= n && n <= MAX_INT ? "number" : "unreadable";
  }
  const unitStart = digits.search(/[^0-9.]/);
  const number = unitStart < 0 ? digits : digits.slice(0, unitStart);
  // A number with two points is a word, such as the version `1.2.3`.
  if (number.split(".").length > 2) return "word";
  if (unitStart < 0) return "number";
  return DURATION_UNITS.has(digits.slice(unitStart)) && !word.startsWith("-") ? "number" : "word";
}

export function lex(text: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;
  while (i < text.length) {
    const start = i;
    const c = charAt(text, i);
    i += c.length;
    if (WHITE_SPACE.test(c)) continue;
    const orEqual = text.charAt(i) === "=";
    let type: TokenType;
    if (c === "(" || c === ")" || c === ",") {
      type = c;
    } else if (c === "~") {
      type = "operator";
    } else if (c === "=" || c === "<" || c === ">") {
      if (orEqual) i++;
      type = "operator";
    } else if (c === "!" && orEqual) {
      i++;
      type = "operator";
    } else if (c === '"' || c === "'" || c === "`") {
      let closed = false;
      while (i < text.length && !closed) {
        const quoted = charAt(text, i);
        i += quoted.length;
        if (quoted === "\\") i += charAt(text, i).length;
        else if (quoted === c) closed = true;
      }
      type = !closed ? "unclosed" : c === "`" ? "backticked" : "string";
    } else if (isWordChar(c)) {
      for (let next = charAt(text, i); isWordChar(next); next = charAt(text, i)) i += next.length;
      type = classifyWord(text.slice(start, i));
    } else {
      type = "unreadable";
    }
    tokens.push({ type, start, end: i });
  }
  return tokens;
}
