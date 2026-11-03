export function formatTechnicalProse(text: string): string {
  if (!text) return "";

  // 1. Wrap calc(...) and var(...) expressions
  let out = text.replace(/(calc\([^)]*(?:\([^)]*\)[^)]*)*\))/g, "`$1`");

  // 2. Wrap standalone var(...) expressions not already inside backticks
  out = out.replace(/(?<![`\w(])(var\([^)]+\))(?![`\w)])/g, (match, _p1, offset, str) => {
    const s = typeof str === "string" ? str : typeof offset === "string" ? offset : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  // 3. Wrap CSS variables `--[a-zA-Z0-9_-]+` not already inside backticks
  out = out.replace(/(?<![`\w-])(--[a-zA-Z0-9_-]+)(?![`\w-])/g, (match, _p1, offset, str) => {
    const s = typeof str === "string" ? str : typeof offset === "string" ? offset : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  // 4. Wrap pixel values like 16px, 12px not inside backticks
  out = out.replace(/(?<![`\w])(\d+px)(?![`\w])/g, (match, _p1, offset, str) => {
    const s = typeof str === "string" ? str : typeof offset === "string" ? offset : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  // 5. Wrap :root not inside backticks
  out = out.replace(/(?<![`\w]):root(?![`\w])/g, (match, offset, str) => {
    const s = typeof str === "string" ? str : typeof offset === "string" ? offset : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  return out;
}

export interface FormattedDeliverable {
  intro?: string;
  items: string[];
  note?: string;
}

export function parseMilestoneDeliverable(raw: string): FormattedDeliverable {
  if (!raw) return { items: [] };

  // Check for (1) ... (2) ... pattern
  if (/\(1\)/.test(raw) && /\(2\)/.test(raw)) {
    const introMatch = raw.match(/^(.*?)(?:—\s*)?\(1\)/);
    const intro = introMatch ? formatTechnicalProse(introMatch[1].trim()) : undefined;

    // Split items by (N)
    const afterIntro = raw.replace(/^(.*?)(?:—\s*)?\(1\)/, "(1)");
    const parts = afterIntro.split(/\s*—?\s*(?:y\s*)?\((\d+)\)\s*/).filter(Boolean);

    const items: string[] = [];
    let note: string | undefined;

    for (let i = 0; i < parts.length; i += 2) {
      const num = parts[i];
      let body = parts[i + 1] ?? "";

      // Check if body ends with an outcome note like "Al cambiar ..." or "Al terminar ..."
      const noteMatch = body.match(/(?:\.\s+)(Al\s+(?:cambiar|terminar|finalizar|probar|hacer)[^.]*\.?)$/i);
      if (noteMatch && i + 2 >= parts.length) {
        body = body.slice(0, noteMatch.index).trim();
        note = formatTechnicalProse(noteMatch[1].trim());
      }

      body = body.replace(/—\s*y\s*$/, "").trim();
      items.push(`(${num}) ${formatTechnicalProse(body)}`);
    }

    return { intro, items, note };
  }

  // If contains newlines
  if (raw.includes("\n")) {
    const lines = raw.split("\n").map((l) => formatTechnicalProse(l.trim())).filter(Boolean);
    return { items: lines };
  }

  return { items: [formatTechnicalProse(raw)] };
}

const inputExpected =
  "CSS con dos bloques en :root: (1) tokens de tipografía — --font-size-base: 16px y --font-size-sm / --font-size-md / --font-size-lg derivados con calc(var(--font-size-base) * multiplicador * var(--type-density)) — y (2) un token único --type-density que gobierna toda la escala. Al cambiar --type-density a 0.85, los tres tamaños derivados se ajustan juntos sin editar ninguna definición.";

console.log(JSON.stringify(parseMilestoneDeliverable(inputExpected), null, 2));
