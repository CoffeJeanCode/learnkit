//! `illustration_kit`: a 2D scientific illustration composed from a closed
//! kit of parametric pieces (blocks, pulleys, inclines, springs, circuit
//! elements…) placed on a coarse grid of SLOTS — never pixel coordinates —
//! joined by typed links (rope, spring, wire, rod, arrow) and annotated with
//! force arrows. The frontend draws every piece, routes the links and keeps
//! labels clear; the model only says what is where. Replaces free-form SVG,
//! the most expensive and error-prone engine.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::domain::notebook::is_placeholder_label;

pub const ILLUSTRATION_PIECES: [&str; 14] =
    ["block", "ball", "pulley", "incline", "wall", "ground", "battery", "resistor", "bulb", "switch", "capacitor", "node", "magnet", "label"];
pub const ILLUSTRATION_LINKS: [&str; 5] = ["rope", "spring", "wire", "rod", "arrow"];
/// Force directions: a compass word or an angle in degrees ("30").
pub const FORCE_DIRECTIONS: [&str; 8] = ["up", "down", "left", "right", "up-left", "up-right", "down-left", "down-right"];
pub const GRID_COLS: u32 = 6;
pub const GRID_ROWS: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationItem {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub label: String,
    /// [column 0-5, row 0-3] of the grid.
    pub slot: [u32; 2],
    /// Short value shown next to it ("2 kg", "10 Ω", "9 V").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Incline angle in degrees (`incline` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    #[serde(default)]
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationLink {
    pub from: String,
    pub to: String,
    #[serde(default = "rope")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

fn rope() -> String {
    "rope".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IllustrationForce {
    pub on: String,
    pub direction: String,
    pub label: String,
    /// Relative length 1-3.
    #[serde(default = "two")]
    pub magnitude: u8,
}

fn two() -> u8 {
    2
}

pub fn illustration_violations(i: usize, items: &[IllustrationItem], links: &[IllustrationLink], forces: &[IllustrationForce]) -> Vec<String> {
    let f = format!("bloque {i}: illustration_kit");
    let mut v = Vec::new();
    if !(1..=12).contains(&items.len()) {
        v.push(format!("{f}: {} piezas, debe tener 1-12", items.len()));
    }
    let mut ids = HashSet::new();
    let mut slots = HashSet::new();
    for it in items {
        if !ids.insert(it.id.as_str()) {
            v.push(format!("{f}: id repetido \"{}\"", it.id));
        }
        if !ILLUSTRATION_PIECES.contains(&it.kind.as_str()) {
            v.push(format!("{f}: pieza \"{}\" fuera del kit ({})", it.kind, ILLUSTRATION_PIECES.join(" ")));
        }
        if it.slot[0] >= GRID_COLS || it.slot[1] >= GRID_ROWS {
            v.push(format!("{f}: \"{}\" está en la casilla {:?}, fuera de la cuadrícula {GRID_COLS}×{GRID_ROWS}", it.id, it.slot));
        } else if !slots.insert(it.slot) && it.kind != "ground" {
            v.push(format!("{f}: dos piezas comparten la casilla {:?} — cada pieza va en su propia casilla", it.slot));
        }
        if it.kind != "node" && (it.label.trim().is_empty() || is_placeholder_label(&it.label)) {
            v.push(format!("{f}: la pieza \"{}\" necesita un label que diga qué es", it.id));
        }
        if it.kind == "incline" && !it.angle.is_some_and(|a| (5.0..=75.0).contains(&a)) {
            v.push(format!("{f}: el plano inclinado \"{}\" necesita angle entre 5 y 75 grados", it.id));
        }
    }
    for l in links {
        if !ids.contains(l.from.as_str()) || !ids.contains(l.to.as_str()) || l.from == l.to {
            v.push(format!("{f}: el enlace {}→{} apunta a piezas que no existen", l.from, l.to));
        }
        if !ILLUSTRATION_LINKS.contains(&l.kind.as_str()) {
            v.push(format!("{f}: enlace \"{}\" debe ser rope | spring | wire | rod | arrow", l.kind));
        }
    }
    if forces.len() > 8 {
        v.push(format!("{f}: {} fuerzas, máximo 8", forces.len()));
    }
    for fo in forces {
        if !ids.contains(fo.on.as_str()) {
            v.push(format!("{f}: la fuerza \"{}\" actúa sobre \"{}\", que no existe", fo.label, fo.on));
        }
        let angle_ok = fo.direction.trim().parse::<f64>().is_ok_and(|a| a.is_finite());
        if !FORCE_DIRECTIONS.contains(&fo.direction.as_str()) && !angle_ok {
            v.push(format!("{f}: la dirección \"{}\" debe ser up/down/left/right/up-left… o un ángulo en grados", fo.direction));
        }
        if fo.label.trim().is_empty() {
            v.push(format!("{f}: toda fuerza necesita label (\"Peso\", \"Normal\", \"T\")"));
        }
        if !(1..=3).contains(&fo.magnitude) {
            v.push(format!("{f}: magnitude de \"{}\" debe ser 1, 2 o 3", fo.label));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: &str, slot: [u32; 2]) -> IllustrationItem {
        IllustrationItem { id: id.into(), kind: kind.into(), label: format!("Pieza {id}x"), slot, value: None, angle: None, explanation: "e".into() }
    }

    #[test]
    fn an_atwood_machine_passes_and_overlaps_go_back() {
        let items = vec![item("p", "pulley", [2, 0]), item("m1", "block", [1, 2]), item("m2", "block", [3, 3])];
        let links = vec![
            IllustrationLink { from: "m1".into(), to: "p".into(), kind: "rope".into(), label: None },
            IllustrationLink { from: "p".into(), to: "m2".into(), kind: "rope".into(), label: None },
        ];
        let forces = vec![IllustrationForce { on: "m1".into(), direction: "down".into(), label: "Peso".into(), magnitude: 2 }];
        assert!(illustration_violations(0, &items, &links, &forces).is_empty(), "{:?}", illustration_violations(0, &items, &links, &forces));
        let clash = vec![item("a", "block", [1, 1]), item("b", "ball", [1, 1])];
        assert!(illustration_violations(0, &clash, &[], &[]).iter().any(|s| s.contains("comparten")));
        let mut slope = item("r", "incline", [0, 3]);
        slope.angle = Some(90.0);
        assert!(illustration_violations(0, &[slope], &[], &[]).iter().any(|s| s.contains("angle")));
    }
}
