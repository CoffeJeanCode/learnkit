/**
 * Formats technical prose by wrapping CSS variables, functions, and keywords in inline backticks
 * if they are not already enclosed in code spans.
 */
function wrapBalancedFunction(text: string, funcName: string): string {
  let result = "";
  let cursor = 0;
  const target = funcName + "(";

  while (cursor < text.length) {
    const idx = text.indexOf(target, cursor);
    if (idx === -1) {
      result += text.slice(cursor);
      break;
    }

    // Check if already inside backticks
    const backticksBefore = (text.slice(0, idx).match(/`/g) || []).length;
    if (backticksBefore % 2 === 1) {
      result += text.slice(cursor, idx + target.length);
      cursor = idx + target.length;
      continue;
    }

    result += text.slice(cursor, idx);
    let depth = 0;
    let end = idx + funcName.length; // points to '('
    for (let i = end; i < text.length; i++) {
      if (text[i] === "(") depth++;
      else if (text[i] === ")") {
        depth--;
        if (depth === 0) {
          end = i + 1;
          break;
        }
      }
    }

    const matched = text.slice(idx, end);
    result += `\`${matched}\``;
    cursor = end;
  }
  return result;
}

export function formatTechnicalProse(text: string): string {
  if (!text) return "";

  // 1. Wrap balanced CSS functions: calc(...) and var(...) and clamp(...)
  let out = wrapBalancedFunction(text, "calc");
  out = wrapBalancedFunction(out, "var");
  out = wrapBalancedFunction(out, "clamp");

  // 2. Wrap CSS variables `--[a-zA-Z0-9_-]+` not already inside backticks
  out = out.replace(/(?<![`\w-])(--[a-zA-Z0-9_-]+)(?![`\w-])/g, (match, _p1, offset, str) => {
    const s = typeof str === "string" ? str : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  // 3. Wrap pixel values like 16px, 12px, 8px not inside backticks
  out = out.replace(/(?<![`\w])(\d+px)(?![`\w])/g, (match, _p1, offset, str) => {
    const s = typeof str === "string" ? str : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  // 4. Wrap :root not inside backticks
  out = out.replace(/(?<![`\w]):root(?![`\w])/g, (match, offset, str) => {
    const s = typeof str === "string" ? str : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  // 5. Wrap numeric scales like "0.75 | 1 | 1.5" or "0.5 | 1 | 1.5"
  out = out.replace(/(?<![`\w])(\d+(?:\.\d+)?\s*\|\s*\d+(?:\.\d+)?\s*\|\s*\d+(?:\.\d+)?)(?![`\w])/g, (match, _p1, offset, str) => {
    const s = typeof str === "string" ? str : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  // 6. Wrap decimal multipliers or ratios (e.g. 0.85, 1.25, 0.75, 1.2) not inside backticks
  out = out.replace(/(?<![`\w])(\b\d+\.\d+\b)(?![`\w])/g, (match, _p1, offset, str) => {
    const s = typeof str === "string" ? str : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  // 7. Wrap standalone CSS unit words like "un px", "en rem"
  out = out.replace(/(?<=\b(?:un|en|el)\s+)(px|rem|em)\b(?![`\w])/g, (match, _p1, offset, str) => {
    const s = typeof str === "string" ? str : out;
    const off = typeof offset === "number" ? offset : 0;
    if ((s.slice(0, off).match(/`/g) || []).length % 2 === 1) return match;
    return `\`${match}\``;
  });

  // 8. Wrap size scale tokens like sm / md / lg
  out = out.replace(/(?<![`\w])(sm\s*\/\s*md\s*\/\s*lg)(?![`\w])/g, (match, _p1, offset, str) => {
    const s = typeof str === "string" ? str : out;
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

/**
 * Parses unstructured or run-on milestone deliverables into clean, readable sections.
 */
export function parseMilestoneDeliverable(raw: string): FormattedDeliverable {
  if (!raw) return { items: [] };

  // Check for (1) ... (2) ... pattern
  if (/\(1\)/.test(raw) && /\(2\)/.test(raw)) {
    const introMatch = raw.match(/^(.*?)(?:—\s*)?\(1\)/);
    const rawIntro = introMatch && introMatch[1].trim() ? introMatch[1].trim().replace(/:\s*$/, "") : undefined;
    const intro = rawIntro ? formatTechnicalProse(rawIntro) : undefined;

    const afterIntro = raw.replace(/^(.*?)(?:—\s*)?\(1\)/, "(1)");
    const parts = afterIntro.split(/\s*—?\s*(?:y\s*)?\((\d+)\)\s*/).filter(Boolean);

    const items: string[] = [];
    let note: string | undefined;

    for (let i = 0; i < parts.length; i += 2) {
      const num = parts[i];
      let body = parts[i + 1] ?? "";

      // Check if body ends with an outcome note like "Al cambiar ..." or "Al terminar ..."
      const noteMatch = body.match(/(?:\.\s+)(Al\s+(?:cambiar|terminar|finalizar|probar|hacer)[\s\S]*)$/i);
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

/**
 * Unpacks constraints whether they are an array or a single string joined with middle dots ` · `.
 */
export function unpackConstraints(constraints: string | readonly string[] | undefined | null): string[] {
  if (!constraints) return [];
  const list = Array.isArray(constraints) ? constraints : [String(constraints)];
  return list
    .flatMap((c) => (typeof c === "string" ? c.split(/\s+[·•\-]\s+|\n+/) : []))
    .map((c) => c.trim())
    .filter((c) => c.length > 0);
}
