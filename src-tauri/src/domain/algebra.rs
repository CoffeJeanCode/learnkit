//! `algebraic_structure`: a finite group given by its KIND and generators —
//! never by a table the model writes. The group, its Cayley table, a subgroup
//! and its cosets are computed (here to validate, in
//! `src/lib/algebra.ts` to draw), so Lagrange's theorem shows up by
//! construction: |G| = |H| · [G : H].
//!
//! Every group is realised as permutations of {0, …, n-1}:
//! - `cyclic` n: Z_n, elements written 0 … n-1 (generator "1");
//! - `dihedral` n: D_n, symmetries of the n-gon, elements e, r, r2, …, s, sr, …;
//! - `symmetric` n: S_n, elements in cycle notation "(1 2 3)" (points 1 … n).

use std::collections::{BTreeSet, HashSet, VecDeque};

pub const GROUP_KINDS: [&str; 3] = ["cyclic", "dihedral", "symmetric"];
pub const MAX_GROUP_ORDER: usize = 24;

pub type Perm = Vec<usize>;

fn compose(a: &Perm, b: &Perm) -> Perm {
    // (a ∘ b)(x) = a(b(x)): apply b first.
    b.iter().map(|&x| a[x]).collect()
}

/// The points the group acts on.
pub fn degree(_kind: &str, n: u32) -> usize {
    n as usize
}

/// One named element as a permutation, or None if the name is not of this group.
pub fn parse_element(kind: &str, n: u32, name: &str) -> Option<Perm> {
    let n = n as usize;
    let name = name.trim();
    let rot = |k: usize| -> Perm { (0..n).map(|x| (x + k) % n).collect() };
    match kind {
        "cyclic" => name.parse::<usize>().ok().filter(|k| *k < n).map(rot),
        "dihedral" => {
            let refl: Perm = (0..n).map(|x| (n - x) % n).collect();
            if name == "e" {
                return Some(rot(0));
            }
            let (has_s, rest) = match name.strip_prefix('s') {
                Some(r) => (true, r),
                None => (false, name),
            };
            let k = if rest.is_empty() && has_s {
                0
            } else if rest == "r" {
                1
            } else {
                rest.strip_prefix('r')?.parse::<usize>().ok().filter(|k| *k < n)?
            };
            let r = rot(k);
            Some(if has_s { compose(&refl, &r) } else { r })
        }
        "symmetric" => {
            let mut p: Perm = (0..n).collect();
            if name == "e" || name == "()" {
                return Some(p);
            }
            // Product of cycles, rightmost applied first: "(1 2)(2 3)".
            let mut cycles: Vec<Vec<usize>> = Vec::new();
            for part in name.split(')').map(str::trim).filter(|s| !s.is_empty()) {
                let inner = part.strip_prefix('(')?;
                let pts: Option<Vec<usize>> = inner.split([' ', ',']).filter(|s| !s.is_empty()).map(|s| s.parse::<usize>().ok().filter(|v| (1..=n).contains(v)).map(|v| v - 1)).collect();
                let pts = pts?;
                if pts.len() < 2 || pts.iter().collect::<HashSet<_>>().len() != pts.len() {
                    return None;
                }
                cycles.push(pts);
            }
            for c in cycles.iter().rev() {
                let mut q: Perm = (0..n).collect();
                for (j, &x) in c.iter().enumerate() {
                    q[x] = c[(j + 1) % c.len()];
                }
                p = compose(&q, &p);
            }
            Some(p)
        }
        _ => None,
    }
}

/// Closure of the generators (BFS); None past `limit` elements.
pub fn generate(gens: &[Perm], n: usize, limit: usize) -> Option<Vec<Perm>> {
    let id: Perm = (0..n).collect();
    let mut seen: BTreeSet<Perm> = [id.clone()].into();
    let mut queue: VecDeque<Perm> = [id].into();
    while let Some(x) = queue.pop_front() {
        for g in gens {
            let y = compose(g, &x);
            if seen.insert(y.clone()) {
                if seen.len() > limit {
                    return None;
                }
                queue.push_back(y);
            }
        }
    }
    Some(seen.into_iter().collect())
}

/// The whole group of a kind.
pub fn whole_group(kind: &str, n: u32) -> Option<Vec<Perm>> {
    let d = degree(kind, n);
    let gens: Vec<Perm> = match kind {
        "cyclic" => vec![parse_element(kind, n, "1")?],
        "dihedral" => vec![parse_element(kind, n, "r")?, parse_element(kind, n, "s")?],
        "symmetric" if n >= 2 => vec![parse_element(kind, n, "(1 2)")?, parse_element(kind, n, &format!("({})", (1..=n).map(|k| k.to_string()).collect::<Vec<_>>().join(" ")))?],
        "symmetric" => vec![(0..d).collect()],
        _ => return None,
    };
    generate(&gens, d, MAX_GROUP_ORDER)
}

pub fn algebra_violations(i: usize, kind: &str, n: u32, subgroup: &[String]) -> Vec<String> {
    let f = format!("bloque {i}: algebraic_structure");
    let mut v = Vec::new();
    if !GROUP_KINDS.contains(&kind) {
        v.push(format!("{f}: group.kind debe ser cyclic | dihedral | symmetric"));
        return v;
    }
    let ok_n = match kind {
        "cyclic" => (2..=24).contains(&n),
        "dihedral" => (3..=12).contains(&n),
        _ => (2..=4).contains(&n),
    };
    if !ok_n {
        v.push(format!("{f}: n = {n} fuera de rango (cyclic 2-24, dihedral 3-12, symmetric 2-4: el grupo debe tener ≤ {MAX_GROUP_ORDER} elementos)"));
        return v;
    }
    let Some(group) = whole_group(kind, n) else {
        v.push(format!("{f}: el grupo tiene más de {MAX_GROUP_ORDER} elementos"));
        return v;
    };
    let mut gens = Vec::new();
    for g in subgroup {
        match parse_element(kind, n, g) {
            Some(p) => gens.push(p),
            None => v.push(format!(
                "{f}: \"{g}\" no es un elemento de este grupo — {}",
                match kind {
                    "cyclic" => "en Z_n escribe un número 0 … n-1",
                    "dihedral" => "en D_n escribe e, r, r2, …, s, sr, sr2, …",
                    _ => "en S_n escribe ciclos como (1 2) o (1 2 3)",
                }
            )),
        }
    }
    if v.is_empty() && !gens.is_empty() {
        let h = generate(&gens, degree(kind, n), MAX_GROUP_ORDER).unwrap_or_default();
        if group.len() % h.len().max(1) != 0 {
            v.push(format!("{f}: el subgrupo calculado no divide al grupo (no debería pasar)"));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_have_their_textbook_orders() {
        assert_eq!(whole_group("cyclic", 12).unwrap().len(), 12);
        assert_eq!(whole_group("dihedral", 4).unwrap().len(), 8);
        assert_eq!(whole_group("symmetric", 3).unwrap().len(), 6);
        assert_eq!(whole_group("symmetric", 4).unwrap().len(), 24);
    }

    #[test]
    fn subgroups_divide_the_group_lagrange() {
        let n = 12;
        let h = generate(&[parse_element("cyclic", n, "4").unwrap()], 12, 24).unwrap();
        assert_eq!(h.len(), 3, "<4> in Z_12 = {{0, 4, 8}}");
        let r2 = generate(&[parse_element("dihedral", 4, "r2").unwrap()], 4, 24).unwrap();
        assert_eq!(r2.len(), 2);
        let s3 = generate(&[parse_element("symmetric", 3, "(1 2 3)").unwrap()], 3, 24).unwrap();
        assert_eq!(s3.len(), 3, "A_3");
        assert!(algebra_violations(0, "dihedral", 4, &["sr".into()]).is_empty());
    }

    #[test]
    fn bad_groups_and_elements_go_back() {
        assert!(!algebra_violations(0, "symmetric", 6, &[]).is_empty(), "S_6 has 720 elements");
        assert!(algebra_violations(0, "cyclic", 12, &["13".into()]).iter().any(|s| s.contains("número 0")));
        assert!(algebra_violations(0, "symmetric", 3, &["(1 4)".into()]).iter().any(|s| s.contains("ciclos")));
        assert!(!algebra_violations(0, "monoid", 3, &[]).is_empty());
    }
}
