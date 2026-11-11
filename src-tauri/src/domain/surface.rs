//! Checks for the `surface_3d` engine: a surface `z = f(x, y)` written in the
//! safe expression grammar (`domain::safe_expr`), with sliders, paths the
//! frontend traces (geodesics, descents, level curves) and an optional
//! volume region. Everything numeric is evaluated HERE before the student
//! sees it: a formula that blows up anywhere in the domain, a flat surface,
//! a path that starts off the surface — each goes back to the model in the
//! same turn. The tracing itself lives in `src/lib/scene3d/surfaceBuild.ts`.

use std::collections::BTreeSet;

use crate::domain::interactive_module::{combos, grid, init_env, parse_with_symbols, variable_violations};
use crate::domain::notebook::{MAX_SURFACE_MARKERS, MAX_SURFACE_PATHS, ModuleVariable, SURFACE_PATH_KINDS, SurfaceMarker, SurfacePath, is_placeholder_label};

pub const MAX_SURFACE_PARAMS: usize = 3;
const MAX_RANGE: f64 = 1e4;
const SAMPLES: usize = 21;

fn range_ok(r: [f64; 2]) -> bool {
    r[0].is_finite() && r[1].is_finite() && r[0] < r[1] && r[0].abs() <= MAX_RANGE && r[1].abs() <= MAX_RANGE
}

fn inside(p: [f64; 2], xr: [f64; 2], yr: [f64; 2]) -> bool {
    p[0] >= xr[0] && p[0] <= xr[1] && p[1] >= yr[0] && p[1] <= yr[1]
}

#[allow(clippy::too_many_arguments)]
pub fn surface_violations(
    i: usize,
    expr: &str,
    x_range: [f64; 2],
    y_range: [f64; 2],
    params: &[ModuleVariable],
    paths: &[SurfacePath],
    markers: &[SurfaceMarker],
    region: Option<[f64; 4]>,
) -> Vec<String> {
    let field = format!("bloque {i}: surface_3d");
    let mut v = Vec::new();
    variable_violations(&format!("{field}.params"), params, MAX_SURFACE_PARAMS, &mut v);
    if params.iter().any(|p| p.sym == "x" || p.sym == "y") {
        v.push(format!("{field}.params: \"x\" e \"y\" son las coordenadas de la superficie; nombra los parámetros de otra forma"));
    }
    if !range_ok(x_range) || !range_ok(y_range) {
        v.push(format!("{field}: xRange y yRange deben ser [min, max] finitos con min < max"));
        return v;
    }
    let mut allowed: BTreeSet<String> = params.iter().map(|p| p.sym.clone()).collect();
    allowed.extend(["x".to_string(), "y".to_string()]);
    if v.is_empty() {
        if let Some(parsed) = parse_with_symbols(&format!("{field}.expr"), expr, &allowed, &mut v) {
            // Finite over the whole grid, at the initial values AND at every
            // slider extreme: a singularity (1/r at r = 0) must be softened
            // in the formula itself (sqrt(x^2 + y^2 + 0.5)), never left in.
            let mut envs = vec![init_env(params)];
            envs.extend(combos(params));
            let (xs, ys) = (grid(x_range[0], x_range[1], SAMPLES), grid(y_range[0], y_range[1], SAMPLES));
            'envs: for (k, env) in envs.into_iter().enumerate() {
                let mut env = env;
                let (mut lo, mut hi) = (f64::MAX, f64::MIN);
                for x in &xs {
                    for y in &ys {
                        env.insert("x".into(), *x);
                        env.insert("y".into(), *y);
                        match parsed.eval(&env) {
                            Ok(z) if z.is_finite() && z.abs() < 1e8 => {
                                lo = lo.min(z);
                                hi = hi.max(z);
                            }
                            _ => {
                                v.push(format!(
                                    "{field}.expr deja de ser finita en x = {x:.3}, y = {y:.3}{} — suaviza la singularidad (p. ej. \
                                     sqrt(x^2 + y^2 + 0.5) en vez de sqrt(x^2 + y^2)) o acota los rangos",
                                    if k == 0 { String::new() } else { " con los sliders en un extremo".to_string() }
                                ));
                                break 'envs;
                            }
                        }
                    }
                }
                if k == 0 && hi - lo < 1e-9 {
                    v.push(format!("{field}.expr es plana en todo el dominio: no muestra ninguna forma"));
                    break;
                }
            }
        }
    }
    if paths.len() > MAX_SURFACE_PATHS {
        v.push(format!("{field}: {} trayectorias, máximo {MAX_SURFACE_PATHS}", paths.len()));
    }
    if markers.len() > MAX_SURFACE_MARKERS {
        v.push(format!("{field}: {} marcadores, máximo {MAX_SURFACE_MARKERS}", markers.len()));
    }
    let mut ids = BTreeSet::new();
    for p in paths {
        if !ids.insert(p.id.as_str()) {
            v.push(format!("{field}: id repetido \"{}\"", p.id));
        }
        if p.label.trim().is_empty() || is_placeholder_label(&p.label) {
            v.push(format!("{field}: la trayectoria \"{}\" necesita un label que diga qué es", p.id));
        }
        if !SURFACE_PATH_KINDS.contains(&p.kind.as_str()) {
            v.push(format!("{field}: kind de \"{}\" debe ser geodesic | descent | level", p.id));
        }
        if !inside(p.start, x_range, y_range) {
            v.push(format!("{field}: la trayectoria \"{}\" empieza fuera de la superficie (start dentro de xRange/yRange)", p.label));
        }
        if p.kind == "geodesic" && (p.direction[0].hypot(p.direction[1]) < 1e-9 || !p.direction.iter().all(|d| d.is_finite())) {
            v.push(format!("{field}: la geodésica \"{}\" necesita una direction distinta de [0, 0]", p.label));
        }
        if p.explanation.trim().is_empty() {
            v.push(format!("{field}: la trayectoria \"{}\" no tiene explanation (qué muestra)", p.label));
        }
    }
    for m in markers {
        if !ids.insert(m.id.as_str()) {
            v.push(format!("{field}: id repetido \"{}\"", m.id));
        }
        if m.label.trim().is_empty() || is_placeholder_label(&m.label) {
            v.push(format!("{field}: el marcador \"{}\" necesita un label concreto", m.id));
        }
        if !inside(m.at, x_range, y_range) {
            v.push(format!("{field}: el marcador \"{}\" está fuera de la superficie", m.label));
        }
    }
    if let Some([x0, x1, y0, y1]) = region {
        if !(x0 < x1 && y0 < y1 && inside([x0, y0], x_range, y_range) && inside([x1, y1], x_range, y_range)) {
            v.push(format!("{field}.region: [x0, x1, y0, y1] con x0 < x1, y0 < y1, dentro de xRange/yRange"));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(sym: &str, min: f64, max: f64, init: f64) -> ModuleVariable {
        ModuleVariable { sym: sym.into(), label: format!("Valor {sym}"), unit: None, min, max, step: None, init }
    }

    fn path(id: &str, kind: &str, start: [f64; 2]) -> SurfacePath {
        SurfacePath { id: id.into(), label: format!("Camino {id}"), kind: kind.into(), start, direction: [1.0, 0.0], explanation: "qué muestra".into() }
    }

    const WELL: &str = "-m / sqrt(x^2 + y^2 + 0.5)";

    #[test]
    fn a_softened_gravity_well_with_geodesics_passes() {
        let v = surface_violations(0, WELL, [-4.0, 4.0], [-4.0, 4.0], &[var("m", 0.5, 3.0, 1.5)], &[path("luz", "geodesic", [-4.0, 1.0])], &[], None);
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn a_singularity_or_a_flat_surface_goes_back() {
        // 1/r at the origin, which the 21-point grid hits.
        let v = surface_violations(0, "-1 / sqrt(x^2 + y^2)", [-4.0, 4.0], [-4.0, 4.0], &[], &[], &[], None);
        assert!(v.iter().any(|s| s.contains("suaviza la singularidad")), "{v:?}");
        let flat = surface_violations(0, "2", [-1.0, 1.0], [-1.0, 1.0], &[], &[], &[], None);
        assert!(flat.iter().any(|s| s.contains("plana")), "{flat:?}");
        let unknown = surface_violations(0, "x * k", [-1.0, 1.0], [-1.0, 1.0], &[], &[], &[], None);
        assert!(!unknown.is_empty(), "an unknown symbol is rejected");
    }

    #[test]
    fn paths_markers_and_regions_must_sit_on_the_surface() {
        let off = surface_violations(0, "x^2 + y^2", [-1.0, 1.0], [-1.0, 1.0], &[], &[path("a", "descent", [5.0, 0.0])], &[], Some([0.0, 2.0, 0.0, 1.0]));
        assert!(off.iter().any(|s| s.contains("empieza fuera")), "{off:?}");
        assert!(off.iter().any(|s| s.contains("region")), "{off:?}");
        let mut still = path("g", "geodesic", [0.0, 0.0]);
        still.direction = [0.0, 0.0];
        let v = surface_violations(0, "x^2 + y^2", [-1.0, 1.0], [-1.0, 1.0], &[], &[still], &[], None);
        assert!(v.iter().any(|s| s.contains("direction")), "{v:?}");
    }
}
