// Edits of a text input by a completion. Servers that count positions in
// characters, as otelo's `/api/complete` does, and the DOM, which counts
// UTF-16 code units, meet here.

/** Text to put in place of the characters from `start` to `end`. */
export interface Suggestion {
  text: string;
  /** Characters, not UTF-16 code units. */
  start: number;
  end: number;
  /** What the text is, such as `field` or `value`. */
  kind: string;
  /** A note shown beside the text, such as a type or a count. */
  detail: string | null;
}

/** The index in characters of the UTF-16 index `index` of `text`. */
export const toChars = (text: string, index: number) => Array.from(text.slice(0, index)).length;

/** The UTF-16 index of the character index `chars` of `text`. */
export function toUtf16(text: string, chars: number): number {
  let index = 0;
  let count = 0;
  for (const c of text) {
    if (count === chars) break;
    index += c.length;
    count++;
  }
  return index;
}

/** The text with `suggestion` in place, and the cursor after it. A space
 * follows the inserted text unless one is there already, so the next
 * completion can start. */
export function applySuggestion(
  text: string,
  suggestion: Suggestion,
): { text: string; cursor: number } {
  const start = toUtf16(text, suggestion.start);
  const end = toUtf16(text, suggestion.end);
  const after = text.slice(end);
  const space = /^[\s),]/.test(after) ? "" : " ";
  const inserted = suggestion.text + space;
  return { text: text.slice(0, start) + inserted + after, cursor: start + inserted.length };
}
