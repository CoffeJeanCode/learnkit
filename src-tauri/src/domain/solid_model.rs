//! Geometry checks for the `solid_model` engine (a CAD-like part built
//! feature by feature, see [`SolidFeature`]). Everything here is plain
//! arithmetic on axis-aligned boxes: enough to tell a part that holds
//! together from one whose pieces float apart or whose hole misses it, without
//! building the mesh (the frontend does that, with the same conventions — see
//! `src/lib/scene3d/solidBuild.ts`).
//!
//! Conventions (shared with the frontend):
//! - CAD frame, Z up. Plane `xy` maps (u, v, n) -> (x, y, z); `xz` -> (x, z, y)
//!   i.e. u = x, v = z, normal +Y; `yz` -> u = y, v = z, normal +X.
//! - `extrude` grows from `offset` along +normal by `depth`.
//! - `cut` / `hole` with a `depth` go INTO the part: from `offset` towards
//!   -normal; without one they cut through everything.
//! - `revolve` spins the profile around the plane's v axis (u = 0).

use crate::domain::notebook::{
    MAX_POLYGON_POINTS, MAX_SOLID_DIM, MAX_SOLID_FEATURES, SOLID_OPS, SOLID_PLANES, SOLID_PROFILES, SolidFeature, is_placeholder_label,
};

/// Largest / smallest measurement allowed in one part: past this, the small
/// feature is a speck nobody can see or click.
pub const MAX_SOLID_RATIO: f64 = 50.0;
const EPS: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb {
    fn union(self, o: Aabb) -> Aabb {
        Aabb { min: [0, 1, 2].map(|k| self.min[k].min(o.min[k])), max: [0, 1, 2].map(|k| self.max[k].max(o.max[k])) }
    }

    /// Overlap or contact (touching faces count: a boss sitting on a plate).
    fn touches(self, o: Aabb) -> bool {
        (0..3).all(|k| self.min[k] <= o.max[k] + EPS && o.min[k] <= self.max[k] + EPS)
    }

    /// Real overlap of volume (a hole that only grazes a face cuts nothing).
    fn overlaps(self, o: Aabb) -> bool {
        (0..3).all(|k| self.min[k] < o.max[k] - EPS && o.min[k] < self.max[k] - EPS)
    }
}

pub fn is_additive(op: &str) -> bool {
    matches!(op, "extrude" | "revolve")
}

/// Axis indices (x=0, y=1, z=2) of the plane's u, v and normal.
pub fn plane_axes(plane: &str) -> [usize; 3] {
    match plane {
        "xz" => [0, 2, 1],
        "yz" => [1, 2, 0],
        _ => [0, 1, 2],
    }
}

/// The profile's extent on its plane, `at` included: (u0, u1, v0, v1).
pub fn profile_extent(f: &SolidFeature) -> Option<(f64, f64, f64, f64)> {
    let [au, av] = f.at;
    let p = &f.profile;
    match p.kind.as_str() {
        "rect" => {
            let (w, h) = (p.w?, p.h?);
            Some((au - w / 2.0, au + w / 2.0, av - h / 2.0, av + h / 2.0))
        }
        "circle" => {
            let r = p.d? / 2.0;
            Some((au - r, au + r, av - r, av + r))
        }
        "polygon" if !p.points.is_empty() => {
            let us = p.points.iter().map(|q| q[0] + au);
            let vs = p.points.iter().map(|q| q[1] + av);
            Some((us.clone().fold(f64::MAX, f64::min), us.fold(f64::MIN, f64::max), vs.clone().fold(f64::MAX, f64::min), vs.fold(f64::MIN, f64::max)))
        }
        _ => None,
    }
}

/// The feature's box in the CAD frame. `through` = half-length used for a
/// cut without depth (it goes through everything).
pub fn feature_box(f: &SolidFeature, through: f64) -> Option<Aabb> {
    let (u0, u1, v0, v1) = profile_extent(f)?;
    let (u_range, n_range) = match f.op.as_str() {
        "revolve" => {
            let r = u0.abs().max(u1.abs());
            ((-r, r), (f.offset - r, f.offset + r))
        }
        "extrude" => ((u0, u1), (f.offset, f.offset + f.depth.unwrap_or(0.0))),
        _ => match f.depth {
            Some(d) => ((u0, u1), (f.offset - d, f.offset)),
            None => ((u0, u1), (-through, through)),
        },
    };
    let [ui, vi, ni] = plane_axes(&f.plane);
    let mut b = Aabb { min: [0.0; 3], max: [0.0; 3] };
    for (axis, (lo, hi)) in [(ui, u_range), (vi, (v0, v1)), (ni, n_range)] {
        b.min[axis] = lo.min(hi);
        b.max[axis] = lo.max(hi);
    }
    Some(b)
}

/// Do two segments (a-b, c-d) properly cross? Shared endpoints don't count.
fn segments_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    o1 * o2 < -EPS && o3 * o4 < -EPS
}

/// A polygon whose edges cross itself has no inside: the extrusion is garbage.
pub fn polygon_is_simple(points: &[[f64; 2]]) -> bool {
    let n = points.len();
    for i in 0..n {
        for j in i + 1..n {
            // Adjacent edges share a vertex; skip them (and the wrap-around pair).
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if segments_cross(points[i], points[(i + 1) % n], points[j], points[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}

/// Everything wrong with a `solid_model`'s features, as messages the model can act on.
pub fn solid_violations(i: usize, features: &[SolidFeature]) -> Vec<String> {
    let mut v = Vec::new();
    if !(1..=MAX_SOLID_FEATURES).contains(&features.len()) {
        v.push(format!("bloque {i}: solid_model tiene {} features, debe tener 1-{MAX_SOLID_FEATURES}", features.len()));
        return v;
    }
    if !features.first().is_some_and(|f| is_additive(&f.op)) {
        v.push(format!(
            "bloque {i}: la primera feature del solid_model debe crear material (extrude o revolve) — un cut o un hole no tiene de dónde quitar"
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let mut sizes: Vec<f64> = Vec::new();
    for f in features {
        let name = if f.label.trim().is_empty() { f.id.as_str() } else { f.label.as_str() };
        if !seen.insert(f.id.as_str()) {
            v.push(format!("bloque {i}: solid_model repite el id \"{}\"", f.id));
        }
        if f.label.trim().is_empty() || is_placeholder_label(&f.label) {
            v.push(format!(
                "bloque {i}: la feature \"{}\" del solid_model necesita un label que diga qué es (\"Base 40×20\", \"Agujero Ø6\"), no un marcador",
                f.id
            ));
        }
        if f.explanation.trim().is_empty() {
            v.push(format!("bloque {i}: la feature \"{name}\" del solid_model no tiene explanation (qué es y por qué está)"));
        }
        if !SOLID_OPS.contains(&f.op.as_str()) || !SOLID_PLANES.contains(&f.plane.as_str()) || !SOLID_PROFILES.contains(&f.profile.kind.as_str()) {
            v.push(format!(
                "bloque {i}: la feature \"{name}\" usa op/plane/profile fuera del catálogo (op: extrude|revolve|cut|hole; plane: xy|xz|yz; profile.kind: rect|circle|polygon)"
            ));
            continue;
        }
        let p = &f.profile;
        let dims: Vec<Option<f64>> = match p.kind.as_str() {
            "rect" => vec![p.w, p.h],
            "circle" => vec![p.d],
            _ => Vec::new(),
        };
        if dims.iter().any(Option::is_none) {
            let needs = if p.kind == "rect" { "w y h" } else { "d" };
            v.push(format!("bloque {i}: el perfil {} de \"{name}\" necesita {needs}", p.kind));
            continue;
        }
        if p.kind == "polygon" {
            if !(3..=MAX_POLYGON_POINTS).contains(&p.points.len()) {
                v.push(format!("bloque {i}: el polígono de \"{name}\" tiene {} puntos, debe tener 3-{MAX_POLYGON_POINTS}", p.points.len()));
                continue;
            }
            if !polygon_is_simple(&p.points) {
                v.push(format!("bloque {i}: el polígono de \"{name}\" se cruza a sí mismo — ordena los puntos recorriendo el contorno"));
                continue;
            }
        }
        let mut measures: Vec<f64> = dims.into_iter().flatten().collect();
        if let Some((u0, u1, v0, v1)) = profile_extent(f) {
            if p.kind == "polygon" {
                measures.extend([u1 - u0, v1 - v0]);
            }
            if f.op == "revolve" && u0 < -EPS {
                v.push(format!(
                    "bloque {i}: el perfil de revolución \"{name}\" cruza su eje (u < 0) — en revolve el perfil va a un solo lado del eje v (u ≥ 0)"
                ));
            }
        }
        match (f.op.as_str(), f.depth) {
            ("extrude", None) => v.push(format!("bloque {i}: la extrusión \"{name}\" necesita depth")),
            ("hole", _) if p.kind != "circle" => v.push(format!("bloque {i}: el agujero \"{name}\" debe tener perfil circle (para otra forma usa cut)")),
            _ => {}
        }
        // A revolve spins 360°: it has no depth (repair drops one the model sent).
        if let (Some(d), false) = (f.depth, f.op == "revolve") {
            measures.push(d);
        }
        if measures.iter().any(|m| !m.is_finite() || *m <= 0.0 || *m > MAX_SOLID_DIM) {
            v.push(format!("bloque {i}: las medidas de \"{name}\" deben ser positivas y ≤ {MAX_SOLID_DIM}"));
            continue;
        }
        if f.at.iter().chain([&f.offset]).any(|c| !c.is_finite() || c.abs() > MAX_SOLID_DIM) {
            v.push(format!("bloque {i}: la posición (at/offset) de \"{name}\" está fuera de ±{MAX_SOLID_DIM}"));
            continue;
        }
        sizes.extend(measures);
    }
    if !v.is_empty() {
        return v;
    }
    if let (Some(max), Some(min)) = (sizes.iter().copied().reduce(f64::max), sizes.iter().copied().reduce(f64::min)) {
        if max / min > MAX_SOLID_RATIO {
            v.push(format!(
                "bloque {i}: el solid_model mezcla medidas de {max} y {min} (más de {MAX_SOLID_RATIO}:1) — la pieza pequeña no se vería; usa medidas proporcionadas"
            ));
        }
    }
    // Assembly: each new piece of material sits on (or in) what exists; each
    // removal actually bites the part.
    let through = MAX_SOLID_DIM * 4.0;
    let mut part: Option<Aabb> = None;
    for f in features {
        let Some(b) = feature_box(f, through) else { continue };
        let name = &f.label;
        if is_additive(&f.op) {
            match part {
                Some(existing) if !existing.touches(b) => v.push(format!(
                    "bloque {i}: \"{name}\" flota separada del resto de la pieza — colócala (at/offset) tocando lo que ya existe"
                )),
                _ => {}
            }
            part = Some(part.map_or(b, |p| p.union(b)));
        } else if let Some(existing) = part {
            if !existing.overlaps(b) {
                v.push(format!("bloque {i}: el {} \"{name}\" no toca la pieza — ajusta at/offset para que quite material de verdad", f.op));
            } else {
                let [ui, vi, _] = plane_axes(&f.plane);
                let covers = |k: usize| b.min[k] <= existing.min[k] + EPS && b.max[k] >= existing.max[k] - EPS;
                if f.depth.is_none() && covers(ui) && covers(vi) {
                    v.push(format!(
                        "bloque {i}: el {} \"{name}\" es tan grande como la pieza y la atraviesa entera — la borraría; hazlo más pequeño que el perfil",
                        f.op
                    ));
                }
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::SolidProfile;

    pub(crate) fn rect(w: f64, h: f64) -> SolidProfile {
        SolidProfile { kind: "rect".into(), w: Some(w), h: Some(h), d: None, points: vec![] }
    }

    pub(crate) fn circle(d: f64) -> SolidProfile {
        SolidProfile { kind: "circle".into(), w: None, h: None, d: Some(d), points: vec![] }
    }

    fn feature(id: &str, op: &str, plane: &str, profile: SolidProfile, at: [f64; 2], offset: f64, depth: Option<f64>) -> SolidFeature {
        SolidFeature {
            id: id.into(),
            label: format!("Pieza {id}"),
            op: op.into(),
            plane: plane.into(),
            profile,
            at,
            offset,
            depth,
            explanation: "qué es".into(),
        }
    }

    fn bracket() -> Vec<SolidFeature> {
        vec![
            feature("base", "extrude", "xy", rect(60.0, 30.0), [0.0, 0.0], 0.0, Some(8.0)),
            feature("pared", "extrude", "xy", rect(8.0, 30.0), [-26.0, 0.0], 8.0, Some(32.0)),
            feature("agujero", "hole", "xy", circle(8.0), [15.0, 0.0], 0.0, None),
        ]
    }

    #[test]
    fn a_coherent_bracket_passes() {
        assert!(solid_violations(0, &bracket()).is_empty(), "{:?}", solid_violations(0, &bracket()));
    }

    #[test]
    fn the_part_must_start_with_material_and_hold_together() {
        let mut f = bracket();
        f.swap(0, 2);
        assert!(solid_violations(0, &f).iter().any(|s| s.contains("debe crear material")));
        let mut floating = bracket();
        floating[1].offset = 50.0;
        assert!(solid_violations(0, &floating).iter().any(|s| s.contains("flota separada")));
    }

    #[test]
    fn a_hole_must_bite_the_part_without_erasing_it() {
        let mut missing = bracket();
        missing[2].at = [200.0, 0.0];
        assert!(solid_violations(0, &missing).iter().any(|s| s.contains("no toca la pieza")));
        let mut huge = bracket();
        huge[2].profile = circle(100.0);
        assert!(solid_violations(0, &huge).iter().any(|s| s.contains("la borraría")));
        let mut square = bracket();
        square[2].profile = rect(5.0, 5.0);
        assert!(solid_violations(0, &square).iter().any(|s| s.contains("perfil circle")));
    }

    #[test]
    fn measures_are_checked() {
        let mut f = bracket();
        f[0].depth = None;
        assert!(solid_violations(0, &f).iter().any(|s| s.contains("necesita depth")));
        let mut speck = bracket();
        speck[2].profile = circle(0.5);
        assert!(solid_violations(0, &speck).iter().any(|s| s.contains("más de 50:1")));
        let mut neg = bracket();
        neg[0].profile = rect(-1.0, 30.0);
        assert!(solid_violations(0, &neg).iter().any(|s| s.contains("positivas")));
        let mut placeholder = bracket();
        placeholder[1].label = "pieza 2".into();
        assert!(solid_violations(0, &placeholder).iter().any(|s| s.contains("no un marcador")));
    }

    #[test]
    fn polygons_must_be_simple_and_revolve_stays_on_one_side_of_its_axis() {
        assert!(polygon_is_simple(&[[0.0, 0.0], [10.0, 0.0], [10.0, 5.0], [0.0, 5.0]]));
        assert!(!polygon_is_simple(&[[0.0, 0.0], [10.0, 5.0], [10.0, 0.0], [0.0, 5.0]]), "a bow-tie crosses itself");
        let bowtie = SolidProfile { kind: "polygon".into(), w: None, h: None, d: None, points: vec![[0.0, 0.0], [10.0, 5.0], [10.0, 0.0], [0.0, 5.0]] };
        let f = vec![feature("p", "extrude", "xy", bowtie, [0.0, 0.0], 0.0, Some(5.0))];
        assert!(solid_violations(0, &f).iter().any(|s| s.contains("se cruza")));
        let crossing = vec![feature("eje", "revolve", "xz", rect(10.0, 30.0), [0.0, 15.0], 0.0, None)];
        assert!(solid_violations(0, &crossing).iter().any(|s| s.contains("cruza su eje")));
        let fine = vec![feature("eje", "revolve", "xz", rect(10.0, 30.0), [5.0, 15.0], 0.0, None)];
        assert!(solid_violations(0, &fine).is_empty(), "{:?}", solid_violations(0, &fine));
    }

    #[test]
    fn a_revolve_ignores_any_depth() {
        let mut vase = vec![feature("vaso", "revolve", "xz", rect(4.0, 60.0), [18.0, 30.0], 0.0, Some(0.0))];
        assert!(solid_violations(0, &vase).is_empty(), "{:?}", solid_violations(0, &vase));
        vase[0].depth = Some(360.0);
        assert!(solid_violations(0, &vase).is_empty(), "an angle in depth is not a measure");
    }

    #[test]
    fn plane_axes_follow_the_shared_convention() {
        let f = feature("b", "extrude", "xz", rect(10.0, 20.0), [0.0, 10.0], 0.0, Some(5.0));
        let b = feature_box(&f, 100.0).unwrap();
        assert_eq!(b.min, [-5.0, 0.0, 0.0]);
        assert_eq!(b.max, [5.0, 5.0, 20.0], "u = x, v = z, normal +Y");
    }
}
