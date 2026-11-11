//! `molecule_3d`: a molecule (or reacting molecules) as atoms + bonds — no
//! coordinates, the frontend lays it out in 3D — with optional mechanism
//! steps that form/break bonds and move charges. Checked here: known
//! elements, bonds that point at real atoms, and VALENCE at every step (a
//! carbon with five bonds is a wrong picture of chemistry, not a style).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::domain::notebook::is_placeholder_label;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoleculeAtom {
    pub id: String,
    pub element: String,
    #[serde(default)]
    pub charge: i8,
    /// Optional name shown instead of the symbol ("Nu⁻", "C1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoleculeBond {
    pub a: String,
    pub b: String,
    #[serde(default = "one")]
    pub order: u8,
}

fn one() -> u8 {
    1
}

/// One frame of a mechanism: bonds that form and break, new charges.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoleculeStep {
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub form: Vec<[String; 2]>,
    #[serde(default, rename = "break")]
    pub break_bonds: Vec<[String; 2]>,
    #[serde(default)]
    pub charges: HashMap<String, i8>,
    #[serde(default)]
    pub highlight: Vec<String>,
}

pub const MAX_ATOMS: usize = 40;
pub const MAX_STEPS: usize = 6;

/// Max bonds (sum of orders) by element and charge — the textbook octet view.
pub fn max_valence(element: &str, charge: i8) -> Option<i32> {
    let base: i32 = match element {
        "H" | "F" | "Cl" | "Br" | "I" | "Li" | "Na" | "K" => 1,
        "O" | "S" | "Mg" | "Ca" => 2,
        "N" | "P" | "B" => 3,
        "C" | "Si" => 4,
        _ => return None,
    };
    // N⁺ / O⁺ gain a bond (ammonium, oxonium); a carbocation or carbanion has 3.
    Some(match (element, charge) {
        ("N" | "O" | "P" | "S", c) if c > 0 => base + c as i32,
        ("C", c) if c != 0 => 3,
        (_, c) if c < 0 => (base - (c.unsigned_abs() as i32)).max(0),
        _ => base,
    })
}

fn key(a: &str, b: &str) -> (String, String) {
    if a <= b { (a.to_string(), b.to_string()) } else { (b.to_string(), a.to_string()) }
}

pub fn molecule_violations(i: usize, atoms: &[MoleculeAtom], bonds: &[MoleculeBond], steps: &[MoleculeStep]) -> Vec<String> {
    let f = format!("bloque {i}: molecule_3d");
    let mut v = Vec::new();
    if !(1..=MAX_ATOMS).contains(&atoms.len()) {
        v.push(format!("{f}: {} átomos, debe tener 1-{MAX_ATOMS}", atoms.len()));
        return v;
    }
    let mut ids = HashSet::new();
    let mut charge: HashMap<String, i8> = HashMap::new();
    let mut element: HashMap<String, String> = HashMap::new();
    for a in atoms {
        if !ids.insert(a.id.as_str()) {
            v.push(format!("{f}: id de átomo repetido \"{}\"", a.id));
        }
        if max_valence(&a.element, 0).is_none() {
            v.push(format!("{f}: elemento \"{}\" fuera del catálogo (H C N O F Cl Br I S P B Si Li Na K Mg Ca)", a.element));
        }
        charge.insert(a.id.clone(), a.charge);
        element.insert(a.id.clone(), a.element.clone());
    }
    if !v.is_empty() {
        return v;
    }
    let mut graph: HashMap<(String, String), u8> = HashMap::new();
    for b in bonds {
        if !ids.contains(b.a.as_str()) || !ids.contains(b.b.as_str()) || b.a == b.b {
            v.push(format!("{f}: el enlace {}-{} apunta a átomos que no existen", b.a, b.b));
            continue;
        }
        if !(1..=3).contains(&b.order) {
            v.push(format!("{f}: el enlace {}-{} tiene orden {}, debe ser 1, 2 o 3", b.a, b.b, b.order));
        }
        if graph.insert(key(&b.a, &b.b), b.order).is_some() {
            v.push(format!("{f}: el enlace {}-{} está repetido", b.a, b.b));
        }
    }
    let check = |graph: &HashMap<(String, String), u8>, charge: &HashMap<String, i8>, when: &str, v: &mut Vec<String>| {
        for (id, el) in &element {
            let used: i32 = graph.iter().filter(|((a, b), _)| a == id || b == id).map(|(_, o)| *o as i32).sum();
            let ch = charge.get(id).copied().unwrap_or(0);
            if let Some(max) = max_valence(el, ch) {
                if used > max {
                    v.push(format!("{f}: {when} el átomo {id} ({el}{}) tiene {used} enlaces y admite {max} — revisa los enlaces o su carga", if ch > 0 { "+" } else if ch < 0 { "−" } else { "" }));
                }
            }
        }
    };
    check(&graph, &charge, "al inicio", &mut v);
    if steps.len() > MAX_STEPS {
        v.push(format!("{f}: {} pasos, máximo {MAX_STEPS}", steps.len()));
    }
    for (s, step) in steps.iter().enumerate() {
        if step.title.trim().is_empty() || is_placeholder_label(&step.title) {
            v.push(format!("{f}: el paso {} necesita un título que diga qué pasa", s + 1));
        }
        for [a, b] in &step.break_bonds {
            if graph.remove(&key(a, b)).is_none() {
                v.push(format!("{f}: el paso {} rompe el enlace {a}-{b}, que no existe en ese momento", s + 1));
            }
        }
        for [a, b] in &step.form {
            if !ids.contains(a.as_str()) || !ids.contains(b.as_str()) || a == b {
                v.push(format!("{f}: el paso {} forma un enlace con átomos inexistentes ({a}-{b})", s + 1));
                continue;
            }
            *graph.entry(key(a, b)).or_insert(0) += 1;
        }
        for (id, c) in &step.charges {
            if !ids.contains(id.as_str()) {
                v.push(format!("{f}: el paso {} cambia la carga de un átomo inexistente ({id})", s + 1));
            }
            charge.insert(id.clone(), *c);
        }
        check(&graph, &charge, &format!("tras el paso {}", s + 1), &mut v);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(id: &str, el: &str) -> MoleculeAtom {
        MoleculeAtom { id: id.into(), element: el.into(), charge: 0, label: None }
    }

    fn bond(a: &str, b: &str) -> MoleculeBond {
        MoleculeBond { a: a.into(), b: b.into(), order: 1 }
    }

    /// CH3Br + OH⁻ → CH3OH + Br⁻ (SN2): C keeps 4 bonds only because the C–Br
    /// bond breaks as the C–O bond forms.
    fn sn2() -> (Vec<MoleculeAtom>, Vec<MoleculeBond>) {
        let mut oh = atom("O", "O");
        oh.charge = -1;
        let atoms = vec![atom("C", "C"), atom("H1", "H"), atom("H2", "H"), atom("H3", "H"), atom("Br", "Br"), oh, atom("Ho", "H")];
        let bonds = vec![bond("C", "H1"), bond("C", "H2"), bond("C", "H3"), bond("C", "Br"), bond("O", "Ho")];
        (atoms, bonds)
    }

    #[test]
    fn an_sn2_step_keeps_every_valence() {
        let (atoms, bonds) = sn2();
        let step = MoleculeStep {
            title: "Ataque por detrás".into(),
            detail: String::new(),
            form: vec![["O".into(), "C".into()]],
            break_bonds: vec![["C".into(), "Br".into()]],
            charges: [("O".to_string(), 0), ("Br".to_string(), -1)].into(),
            highlight: vec![],
        };
        assert!(molecule_violations(0, &atoms, &bonds, &[step]).is_empty());
    }

    #[test]
    fn a_pentavalent_carbon_or_a_phantom_bond_goes_back() {
        let (atoms, bonds) = sn2();
        let only_form = MoleculeStep {
            title: "Ataque".into(),
            detail: String::new(),
            form: vec![["O".into(), "C".into()]],
            break_bonds: vec![],
            charges: [("O".to_string(), 0)].into(),
            highlight: vec![],
        };
        let v = molecule_violations(0, &atoms, &bonds, &[only_form]);
        assert!(v.iter().any(|s| s.contains("tiene 5 enlaces")), "{v:?}");
        let phantom = MoleculeStep { title: "Rompe".into(), detail: String::new(), form: vec![], break_bonds: vec![["C".into(), "O".into()]], charges: HashMap::new(), highlight: vec![] };
        assert!(molecule_violations(0, &atoms, &bonds, &[phantom]).iter().any(|s| s.contains("no existe")));
        assert!(molecule_violations(0, &[atom("X", "Xx")], &[], &[]).iter().any(|s| s.contains("catálogo")));
    }
}
