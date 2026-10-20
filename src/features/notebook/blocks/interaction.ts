// Pure helpers behind the notebook's interactive mechanics (cloze micro-steps,
// causal ordering, "bug bounty" line picking). Kept free of React so
// `scripts/check-rendering.tsx` can exercise them directly.

/** Case/space-insensitive form used to compare a typed cloze token. */
export const normalizeToken = (s: string): string => s.trim().toLowerCase().replace(/\s+/g, " ");

export const tokenMatches = (typed: string, expected: string): boolean => normalizeToken(typed) === normalizeToken(expected);

/** A copy of `items` with the element at `index` moved one slot up (-1) or
 *  down (+1). Out-of-range moves return the list unchanged. */
export function moveItem<T>(items: readonly T[], index: number, delta: -1 | 1): T[] {
  const target = index + delta;
  const next = [...items];
  if (index < 0 || index >= next.length || target < 0 || target >= next.length) return next;
  [next[index], next[target]] = [next[target]!, next[index]!];
  return next;
}

/** Stable key for an answer, to remember attempts that already failed so the
 *  same guess can't be submitted twice. */
export const answerKey = (parts: readonly (string | boolean | null)[]): string => JSON.stringify(parts);

/** Lines of a flawed snippet, 1-based in the UI. A trailing newline does not
 *  add a phantom empty line (the backend counts lines the same way). */
export function splitSnippetLines(snippet: string): string[] {
  const lines = snippet.replace(/\r\n/g, "\n").split("\n");
  if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop();
  return lines;
}
