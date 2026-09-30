// The colors of a query in the query input. The app says what each token of
// its language is, and this picks the color and cuts the text to draw.

/** What a token is, which picks its color. A word is one still being typed,
 * which may become a field or a keyword, such as the `an` of `and`. A number is an
 * integer, a float, or a duration. */
export type TokenKind =
  | "field"
  | "word"
  | "operator"
  | "keyword"
  | "string"
  | "number"
  | "boolean"
  | "punctuation"
  | "invalid";

/** The token from the UTF-16 index `start` to `end` of a query. */
export interface QueryToken {
  start: number;
  end: number;
  kind: TokenKind;
}

/** The tokens of a query, in the order of the text and not overlapping. */
export type Highlight = (query: string) => QueryToken[];

// A field is semibold, which in a monospace font is as wide as regular, so
// the text stays on the text of the input under it. Its dotted underline
// says that the pointer finds help on it.
const tokenClasses: Record<TokenKind, string> = {
  field: "font-semibold text-ink underline decoration-muted decoration-dotted underline-offset-4",
  word: "text-ink",
  operator: "text-muted",
  keyword: "text-syntax-keyword",
  string: "text-syntax-string",
  number: "text-syntax-number",
  boolean: "text-syntax-boolean",
  punctuation: "text-muted",
  invalid: "text-error underline decoration-wavy",
};

/** A run of the text: a token, or what lies between two. */
export interface Piece {
  text: string;
  token: QueryToken | undefined;
}

export const pieceClass = (piece: Piece) => piece.token && tokenClasses[piece.token.kind];

/** The whole of `text` as pieces: its tokens, and what lies between them. */
export function pieces(text: string, tokens: QueryToken[]): Piece[] {
  const out: Piece[] = [];
  let end = 0;
  for (const token of tokens) {
    if (token.start > end) out.push({ text: text.slice(end, token.start), token: undefined });
    out.push({ text: text.slice(token.start, token.end), token });
    end = token.end;
  }
  if (end < text.length) out.push({ text: text.slice(end), token: undefined });
  return out;
}
