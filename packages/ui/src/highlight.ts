const TOKEN_CLASSES: Record<TokenKind, string> = {
  // Semibold is as wide as regular in a monospace font, so a field stays on
  // the text of the input under it.
  field: "font-semibold text-ink underline decoration-muted decoration-dotted underline-offset-4",
  undecided: "text-ink",
  operator: "text-muted",
  keyword: "text-syntax-keyword",
  string: "text-syntax-string",
  number: "text-syntax-number",
  boolean: "text-syntax-boolean",
  punctuation: "text-muted",
  invalid: "text-error underline decoration-wavy",
};

export type TokenKind =
  | "field"
  // A word still being typed, which may become a field or a keyword.
  | "undecided"
  | "operator"
  | "keyword"
  | "string"
  | "number"
  | "boolean"
  | "punctuation"
  | "invalid";

export interface QueryToken {
  readonly start: number;
  readonly end: number;
  readonly kind: TokenKind;
}

export interface Piece {
  readonly text: string;
  readonly token: QueryToken | undefined;
}

export function splitIntoPieces(text: string, tokens: ReadonlyArray<QueryToken>): Piece[] {
  const pieces: Piece[] = [];
  let end = 0;
  for (const token of tokens) {
    if (token.start > end) pieces.push({ text: text.slice(end, token.start), token: undefined });
    pieces.push({ text: text.slice(token.start, token.end), token });
    end = token.end;
  }
  if (end < text.length) pieces.push({ text: text.slice(end), token: undefined });
  return pieces;
}

export const pickPieceClass = (piece: Piece) => piece.token && TOKEN_CLASSES[piece.token.kind];
