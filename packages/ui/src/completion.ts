export interface Suggestion {
  readonly text: string;
  // Characters, not UTF-16 code units.
  readonly start: number;
  readonly end: number;
  readonly kind: string;
  readonly detail: string | null;
}

export const toChars = (text: string, utf16Index: number) =>
  Array.from(text.slice(0, utf16Index)).length;

export function toUtf16(text: string, charIndex: number): number {
  let utf16Index = 0;
  let charCount = 0;
  for (const char of text) {
    if (charCount === charIndex) break;
    utf16Index += char.length;
    charCount++;
  }
  return utf16Index;
}

interface AppliedSuggestion {
  readonly text: string;
  readonly cursor: number;
}

export function applySuggestion(text: string, suggestion: Suggestion): AppliedSuggestion {
  const start = toUtf16(text, suggestion.start);
  const end = toUtf16(text, suggestion.end);
  const after = text.slice(end);
  // A space follows unless one is there, so the next completion can start.
  const space = /^[\s),]/.test(after) ? "" : " ";
  const inserted = suggestion.text + space;
  return { text: text.slice(0, start) + inserted + after, cursor: start + inserted.length };
}
