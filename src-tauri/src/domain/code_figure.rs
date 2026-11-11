//! `code_figure` (tier 2): a figure written as a small JS program against the
//! figure SDK (`fig.*`, see `src/features/notebook/blocks/codeFigureSdk.ts`),
//! for what no declarative engine covers. Only the ESCALATION model may emit
//! it (checked in `notebook_service::generation`), and it runs in a sandboxed
//! iframe with no network and no access to the app. This module is the static
//! gate before that: size, required SDK use, and a deny-list of every API that
//! could reach outside the canvas.

pub const MAX_CODE_CHARS: usize = 12_000;

/// Tokens that never belong in a figure: network, storage, the parent page,
/// dynamic code, workers, timers that outlive the figure's own loop.
pub const FORBIDDEN: [&str; 25] = [
    "fetch", "xmlhttprequest", "websocket", "eventsource", "import(", "import ", "require(", "eval(", "new function",
    "document.cookie", "localstorage", "sessionstorage", "indexeddb", "parent.", "top.", "window.open", "opener", "postmessage",
    "navigator.", "location", "<script", "importscripts", "worker(", "sharedarraybuffer", "document.write",
];

pub fn code_figure_violations(i: usize, code: &str, height: u32) -> Vec<String> {
    let f = format!("bloque {i}: code_figure");
    let mut v = Vec::new();
    if code.trim().is_empty() {
        v.push(format!("{f}: code vacío"));
        return v;
    }
    if code.chars().count() > MAX_CODE_CHARS {
        v.push(format!("{f}: el código tiene más de {MAX_CODE_CHARS} caracteres — simplifica la figura"));
    }
    if !code.contains("fig.") {
        v.push(format!("{f}: el código debe dibujar con el SDK `fig` (fig.ctx, fig.onFrame, fig.slider…)"));
    }
    // Case-insensitive, whitespace-insensitive for the call forms ("eval (").
    let squashed: String = code.to_lowercase().chars().filter(|c| !c.is_whitespace() || *c == ' ').collect();
    let compact = squashed.replace(" (", "(");
    // A token counts only at a word start ("allocation" is not "location").
    let has = |token: &str| {
        compact.match_indices(token).any(|(at, _)| at == 0 || !compact[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_'))
    };
    for token in FORBIDDEN {
        if has(token) {
            v.push(format!("{f}: el código usa \"{}\", que la figura no puede usar (sin red, sin almacenamiento, sin la página padre, sin código dinámico)", token.trim()));
        }
    }
    if !(200..=640).contains(&height) {
        v.push(format!("{f}: height {height} fuera de 200-640"));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = "const k = fig.slider({ label: 'Masa', min: 1, max: 5, value: 2 });\nfig.onFrame((t) => { const c = fig.ctx; c.clearRect(0, 0, fig.width, fig.height); c.fillText('m = ' + k(), 20, 20); });";

    #[test]
    fn a_plain_sdk_figure_passes() {
        assert!(code_figure_violations(0, OK, 360).is_empty());
        let words = format!("{OK}
const allocation = 3; const draw = function () {{ return allocation; }}; // relocation");
        assert!(code_figure_violations(0, &words, 360).is_empty(), "{:?}", code_figure_violations(0, &words, 360));
    }

    #[test]
    fn anything_that_reaches_outside_the_canvas_goes_back() {
        for bad in ["fetch('https://x')", "window.parent.document", "eval ('1')", "new Function('x')", "localStorage.setItem('a','b')", "top.location='x'", "import('x')"] {
            let v = code_figure_violations(0, &format!("{OK}\n{bad}"), 360);
            assert!(!v.is_empty(), "{bad} must be rejected");
        }
        assert!(code_figure_violations(0, "const x = 1;", 360).iter().any(|s| s.contains("SDK")));
        assert!(!code_figure_violations(0, &"fig.ctx;".repeat(2000), 360).is_empty(), "too long");
    }
}
