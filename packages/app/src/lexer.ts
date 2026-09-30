const WHITE_SPACE_PATTERN = /\p{White_Space}/u;
const WORD_CHAR_PATTERN = /[\p{Alphabetic}\p{N}_.\-/:@]/u;
const KEYWORDS = new Set(["and", "or", "not"]);
const DURATION_UNITS = new Set(["ns", "us", "µs", "ms", "s", "m", "h", "d"]);
const MAX_INT = 2n ** 63n - 1n;
const MIN_INT = -(2n ** 63n);

export type TokenType =
  | "word"
  | "number"
  | "string"
  | "backticked"
  | "unclosed"
  | "("
  | ")"
  | ","
  | "operator"
  | "unreadable";

export interface Token {
  readonly type: TokenType;
  readonly start: number;
  readonly end: number;
}

// A port of `lex_tokens` in crates/query/src/lexer.rs. The test of
// highlight.ts holds both to the snapshots of the Rust one.
export function lexQuery(query: string): Token[] {
  const tokens: Token[] = [];
  let index = 0;
  while (index < query.length) {
    const start = index;
    const char = readCharAt(query, index);
    index += char.length;
    if (WHITE_SPACE_PATTERN.test(char)) continue;
    const equalsFollows = query.charAt(index) === "=";
    let type: TokenType;
    if (char === "(" || char === ")" || char === ",") {
      type = char;
    } else if (char === "~") {
      type = "operator";
    } else if (char === "=" || char === "<" || char === ">") {
      if (equalsFollows) index++;
      type = "operator";
    } else if (char === "!" && equalsFollows) {
      index++;
      type = "operator";
    } else if (char === '"' || char === "'" || char === "`") {
      let closed = false;
      while (index < query.length && !closed) {
        const quoted = readCharAt(query, index);
        index += quoted.length;
        if (quoted === "\\") index += readCharAt(query, index).length;
        else if (quoted === char) closed = true;
      }
      type = !closed ? "unclosed" : char === "`" ? "backticked" : "string";
    } else if (isWordChar(char)) {
      for (let next = readCharAt(query, index); isWordChar(next); next = readCharAt(query, index)) {
        index += next.length;
      }
      type = classifyWord(query.slice(start, index));
    } else {
      type = "unreadable";
    }
    tokens.push({ type, start, end: index });
  }
  return tokens;
}

export const isWordChar = (char: string) => WORD_CHAR_PATTERN.test(char);

export const isKeyword = (word: string) => KEYWORDS.has(word.toLowerCase());

function readCharAt(text: string, index: number): string {
  const codePoint = text.codePointAt(index);
  return codePoint === undefined ? "" : String.fromCodePoint(codePoint);
}

function classifyWord(word: string): TokenType {
  const digits = word.startsWith("-") ? word.slice(1) : word;
  if (!/^[0-9]/.test(digits)) return "word";
  if (/^[0-9]+$/.test(digits)) {
    const integer = BigInt(word);
    return MIN_INT <= integer && integer <= MAX_INT ? "number" : "unreadable";
  }
  const unitStart = digits.search(/[^0-9.]/);
  const number = unitStart < 0 ? digits : digits.slice(0, unitStart);
  // A number with two points is a word, such as the version `1.2.3`.
  if (number.split(".").length > 2) return "word";
  if (unitStart < 0) return "number";
  return DURATION_UNITS.has(digits.slice(unitStart)) && !word.startsWith("-") ? "number" : "word";
}
