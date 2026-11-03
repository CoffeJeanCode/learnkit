//! Deterministic validation of `StaticVisualSpec::InteractiveModule`.
//!
//! The model fills a small typed spec; this module decides whether it WORKS:
//! every formula parses, only uses declared symbols, stays finite over the
//! slider ranges, and any "reach this value" challenge is actually reachable.
//! Each violation is a Spanish sentence addressed to the generator (it is
//! quoted back on the retry), so it names the field and the fix.

use std::collections::{BTreeSet, HashMap};

use crate::domain::notebook::{InteractiveModuleSpec, ModuleChallenge, ModuleVariable};
use crate::domain::safe_expr::Expr;

pub const MAX_VARIABLES: usize = 4;
pub const MAX_PARAMS: usize = 3;
pub const MAX_VECTORS: usize = 4;
pub const MAX_PARTICLES: u32 = 60;
pub const MAX_CHANNELS: u32 = 8;
pub const MAX_TIMELINE_EVENTS: usize = 8;
/// Points sampled per variable when checking that a target is reachable.
const GRID: usize = 9;

fn valid_symbol(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic()) && s.chars().count() <= 16 && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn variable_violations(field: &str, vars: &[ModuleVariable], max: usize, out: &mut Vec<String>) {
    if vars.len() > max {
        out.push(format!("{field}: máximo {max} variables, hay {}", vars.len()));
    }
    let mut seen = BTreeSet::new();
    for (i, v) in vars.iter().enumerate() {
        let at = format!("{field}[{i}] (\"{}\")", v.sym);
        if !valid_symbol(&v.sym) {
            out.push(format!("{at}: `sym` debe ser una letra seguida de letras/dígitos/_ (máx. 16), p. ej. \"k\" o \"masa\""));
        } else if matches!(v.sym.as_str(), "pi" | "e") {
            out.push(format!("{at}: `{}` está reservado (constante); usa otro símbolo", v.sym));
        } else if !seen.insert(v.sym.clone()) {
            out.push(format!("{at}: símbolo repetido"));
        }
        if v.label.trim().is_empty() {
            out.push(format!("{at}: `label` vacío"));
        }
        if !(v.min.is_finite() && v.max.is_finite() && v.init.is_finite()) || v.min >= v.max {
            out.push(format!("{at}: se requiere min < max con valores finitos (min={}, max={})", v.min, v.max));
        } else if v.init < v.min || v.init > v.max {
            out.push(format!("{at}: init={} queda fuera de [{}, {}]", v.init, v.min, v.max));
        }
        if let Some(step) = v.step {
            if !(step.is_finite() && step > 0.0) || (v.max > v.min && step > v.max - v.min) {
                out.push(format!("{at}: `step` debe ser > 0 y no mayor que el rango"));
            }
        }
    }
}

/// `n` evenly spaced values over `[min, max]` (inclusive).
fn grid(min: f64, max: f64, n: usize) -> Vec<f64> {
    (0..n).map(|i| min + (max - min) * i as f64 / (n - 1) as f64).collect()
}

/// Every combination of grid values for `vars` (bounded: <= GRID^MAX_VARIABLES).
fn combos(vars: &[ModuleVariable]) -> Vec<HashMap<String, f64>> {
    let mut acc: Vec<HashMap<String, f64>> = vec![HashMap::new()];
    for v in vars {
        let values = grid(v.min, v.max, GRID);
        acc = acc
            .into_iter()
            .flat_map(|base| {
                values.iter().map(move |x| {
                    let mut m = base.clone();
                    m.insert(v.sym.clone(), *x);
                    m
                })
            })
            .collect();
    }
    acc
}

fn init_env(vars: &[ModuleVariable]) -> HashMap<String, f64> {
    vars.iter().map(|v| (v.sym.clone(), v.init)).collect()
}

fn parse_with_symbols(field: &str, src: &str, allowed: &BTreeSet<String>, out: &mut Vec<String>) -> Option<Expr> {
    match Expr::parse(src) {
        Err(e) => {
            out.push(format!("{field}: fórmula inválida (\"{src}\"): {e}. Usa solo + - * / ^ ( ), números, símbolos declarados y sin/cos/tan/sqrt/abs/ln/log/exp/min/max"));
            None
        }
        Ok(expr) => {
            let unknown: Vec<String> = expr.symbols().difference(allowed).cloned().collect();
            if unknown.is_empty() {
                Some(expr)
            } else {
                out.push(format!(
                    "{field}: la fórmula usa {} que no están declarados (declarados: {})",
                    unknown.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", "),
                    if allowed.is_empty() { "ninguno".to_string() } else { allowed.iter().cloned().collect::<Vec<_>>().join(", ") }
                ));
                None
            }
        }
    }
}

/// Violations of one interactive module (empty = it works).
pub fn module_violations(field: &str, spec: &InteractiveModuleSpec, challenge: Option<&ModuleChallenge>) -> Vec<String> {
    let mut v = Vec::new();
    match spec {
        InteractiveModuleSpec::RelationExplorer { output, variables } => {
            if variables.is_empty() {
                v.push(format!("{field}.variables: relation_explorer necesita 1-{MAX_VARIABLES} variables"));
            }
            variable_violations(&format!("{field}.variables"), variables, MAX_VARIABLES, &mut v);
            if !valid_symbol(&output.sym) || variables.iter().any(|x| x.sym == output.sym) {
                v.push(format!("{field}.output.sym (\"{}\"): debe ser un símbolo válido distinto de las variables", output.sym));
            }
            if output.label.trim().is_empty() {
                v.push(format!("{field}.output.label vacío"));
            }
            let allowed: BTreeSet<String> = variables.iter().map(|x| x.sym.clone()).collect();
            if v.is_empty() {
                if let Some(expr) = parse_with_symbols(&format!("{field}.output.expr"), &output.expr, &allowed, &mut v) {
                    let check = |label: &str, env: &HashMap<String, f64>, v: &mut Vec<String>| match expr.eval(env) {
                        Ok(r) if r.is_finite() => {}
                        _ => v.push(format!("{field}.output.expr: la fórmula no da un número finito {label} — revisa divisiones entre 0 o raíces negativas dentro de los rangos")),
                    };
                    check("con los valores iniciales", &init_env(variables), &mut v);
                    if v.is_empty() {
                        for env in combos(variables) {
                            if !expr.eval(&env).is_ok_and(f64::is_finite) {
                                v.push(format!("{field}.output.expr: la fórmula deja de ser finita dentro del rango de los sliders ({env:?}) — acota los rangos"));
                                break;
                            }
                        }
                    }
                }
            }
        }
        InteractiveModuleSpec::FunctionPlot { expr, domain, params, .. } => {
            variable_violations(&format!("{field}.params"), params, MAX_PARAMS, &mut v);
            if params.iter().any(|p| p.sym == "x") {
                v.push(format!("{field}.params: \"x\" es la variable independiente; nombra el parámetro de otra forma"));
            }
            if !(domain[0].is_finite() && domain[1].is_finite()) || domain[0] >= domain[1] {
                v.push(format!("{field}.domain: se requiere [min, max] finito con min < max"));
            }
            let mut allowed: BTreeSet<String> = params.iter().map(|p| p.sym.clone()).collect();
            allowed.insert("x".to_string());
            if v.is_empty() {
                if let Some(parsed) = parse_with_symbols(&format!("{field}.expr"), expr, &allowed, &mut v) {
                    let mut env = init_env(params);
                    let finite = grid(domain[0], domain[1], 25)
                        .into_iter()
                        .filter(|x| {
                            env.insert("x".to_string(), *x);
                            parsed.eval(&env).is_ok_and(f64::is_finite)
                        })
                        .count();
                    if finite < 20 {
                        v.push(format!("{field}.expr: la función solo es finita en {finite} de 25 puntos del dominio — ajusta el dominio para evitar asíntotas"));
                    }
                }
            }
        }
        InteractiveModuleSpec::VectorSum { vectors, .. } => {
            if vectors.len() < 2 || vectors.len() > MAX_VECTORS {
                v.push(format!("{field}.vectors: vector_sum necesita 2-{MAX_VECTORS} vectores, hay {}", vectors.len()));
            }
            for (i, vec) in vectors.iter().enumerate() {
                if vec.label.trim().is_empty() {
                    v.push(format!("{field}.vectors[{i}].label vacío"));
                }
                if !(vec.x.is_finite() && vec.y.is_finite()) || vec.x.abs() > 10.0 || vec.y.abs() > 10.0 {
                    v.push(format!("{field}.vectors[{i}]: x e y deben estar entre -10 y 10 (unidades de la cuadrícula)"));
                }
            }
        }
        InteractiveModuleSpec::ParticleGradient { high_label, low_label, high_count, low_count, has_barrier, channels } => {
            if high_label.trim().is_empty() || low_label.trim().is_empty() {
                v.push(format!("{field}: highLabel y lowLabel no pueden estar vacíos"));
            }
            if *high_count > MAX_PARTICLES || *low_count > MAX_PARTICLES {
                v.push(format!("{field}: highCount/lowCount máximo {MAX_PARTICLES} partículas"));
            }
            if high_count <= low_count {
                v.push(format!("{field}: highCount ({high_count}) debe ser mayor que lowCount ({low_count}) — si no, no hay gradiente"));
            }
            if *channels > MAX_CHANNELS {
                v.push(format!("{field}.channels: máximo {MAX_CHANNELS}"));
            }
            if *has_barrier && *channels == 0 {
                v.push(format!("{field}: con barrera (hasBarrier) pon al menos 1 canal; sin canales nada cruza"));
            }
        }
        InteractiveModuleSpec::Timeline { events } => {
            if events.len() < 2 || events.len() > MAX_TIMELINE_EVENTS {
                v.push(format!("{field}.events: timeline necesita 2-{MAX_TIMELINE_EVENTS} eventos, hay {}", events.len()));
            }
            for (i, e) in events.iter().enumerate() {
                if e.label.trim().is_empty() || e.detail.trim().is_empty() {
                    v.push(format!("{field}.events[{i}]: label y detail no pueden estar vacíos"));
                }
            }
        }
    }
    if let Some(ch) = challenge {
        v.extend(challenge_violations(field, spec, ch));
    }
    v
}

fn challenge_violations(field: &str, spec: &InteractiveModuleSpec, ch: &ModuleChallenge) -> Vec<String> {
    let mut v = Vec::new();
    let at = format!("{field}.challenge");
    if ch.prompt.trim().is_empty() {
        v.push(format!("{at}.prompt vacío"));
    }
    if !ch.target.is_finite() || !ch.tolerance.is_finite() || ch.tolerance < 0.0 {
        v.push(format!("{at}: target/tolerance deben ser finitos y tolerance >= 0"));
        return v;
    }
    match spec {
        InteractiveModuleSpec::RelationExplorer { output, variables } => {
            if let Some(var) = variables.iter().find(|x| x.sym == ch.sym) {
                if ch.target < var.min - ch.tolerance || ch.target > var.max + ch.tolerance {
                    v.push(format!("{at}: target {} queda fuera del slider \"{}\" [{}, {}]", ch.target, var.sym, var.min, var.max));
                }
            } else if ch.sym == output.sym {
                if let Ok(expr) = Expr::parse(&output.expr) {
                    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                    for env in combos(variables) {
                        if let Ok(r) = expr.eval(&env) {
                            if r.is_finite() {
                                lo = lo.min(r);
                                hi = hi.max(r);
                            }
                        }
                    }
                    if lo.is_finite() && (ch.target < lo - ch.tolerance || ch.target > hi + ch.tolerance) {
                        v.push(format!("{at}: target {} no es alcanzable; \"{}\" va de {lo:.4} a {hi:.4} con los sliders", ch.target, output.sym));
                    }
                }
            } else {
                v.push(format!("{at}.sym (\"{}\") no es una variable ni la salida (\"{}\")", ch.sym, output.sym));
            }
        }
        InteractiveModuleSpec::FunctionPlot { params, .. } => match params.iter().find(|p| p.sym == ch.sym) {
            Some(p) if ch.target >= p.min - ch.tolerance && ch.target <= p.max + ch.tolerance => {}
            Some(p) => v.push(format!("{at}: target {} queda fuera del slider \"{}\" [{}, {}]", ch.target, p.sym, p.min, p.max)),
            None => v.push(format!("{at}.sym (\"{}\") no es un parámetro de la función", ch.sym)),
        },
        _ => v.push(format!("{at}: solo relation_explorer y function_plot admiten `challenge`")),
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::{ModuleOutput, TimelineEvent};

    fn var(sym: &str, min: f64, max: f64, init: f64) -> ModuleVariable {
        ModuleVariable { sym: sym.into(), label: sym.to_uppercase(), unit: None, min, max, step: None, init }
    }

    fn hooke() -> InteractiveModuleSpec {
        InteractiveModuleSpec::RelationExplorer {
            output: ModuleOutput { sym: "F".into(), label: "Fuerza".into(), unit: Some("N".into()), expr: "k * x".into() },
            variables: vec![var("k", 10.0, 200.0, 50.0), var("x", 0.0, 0.5, 0.1)],
        }
    }

    #[test]
    fn a_sound_relation_passes() {
        assert!(module_violations("visualAid", &hooke(), None).is_empty());
    }

    #[test]
    fn unknown_symbol_and_bad_syntax_are_named() {
        let InteractiveModuleSpec::RelationExplorer { output, variables } = hooke() else { unreachable!() };
        let unknown = InteractiveModuleSpec::RelationExplorer { output: ModuleOutput { expr: "k * z".into(), ..output.clone() }, variables: variables.clone() };
        let v = module_violations("visualAid", &unknown, None);
        assert!(v.iter().any(|m| m.contains("\"z\"") && m.contains("no están declarados")), "{v:?}");
        let bad = InteractiveModuleSpec::RelationExplorer { output: ModuleOutput { expr: "F = k * x".into(), ..output }, variables };
        assert!(module_violations("visualAid", &bad, None).iter().any(|m| m.contains("fórmula inválida")));
    }

    #[test]
    fn range_and_init_are_checked() {
        let spec = InteractiveModuleSpec::RelationExplorer {
            output: ModuleOutput { sym: "y".into(), label: "y".into(), unit: None, expr: "a".into() },
            variables: vec![var("a", 5.0, 5.0, 5.0), var("b", 0.0, 1.0, 9.0)],
        };
        let v = module_violations("visualAid", &spec, None);
        assert!(v.iter().any(|m| m.contains("min < max")), "{v:?}");
        assert!(v.iter().any(|m| m.contains("fuera de")), "{v:?}");
    }

    #[test]
    fn division_by_zero_inside_the_slider_range_is_rejected() {
        let spec = InteractiveModuleSpec::RelationExplorer {
            output: ModuleOutput { sym: "I".into(), label: "Corriente".into(), unit: None, expr: "V / R".into() },
            variables: vec![var("V", 1.0, 12.0, 5.0), var("R", 0.0, 100.0, 10.0)],
        };
        assert!(module_violations("visualAid", &spec, None).iter().any(|m| m.contains("deja de ser finita")));
    }

    #[test]
    fn unreachable_challenge_target_is_rejected() {
        let ch = ModuleChallenge { prompt: "Llega a 1000 N".into(), sym: "F".into(), target: 1000.0, tolerance: 1.0 };
        assert!(module_violations("visualAid", &hooke(), Some(&ch)).iter().any(|m| m.contains("no es alcanzable")));
        let ok = ModuleChallenge { prompt: "Llega a 10 N".into(), sym: "F".into(), target: 10.0, tolerance: 0.5 };
        assert!(module_violations("visualAid", &hooke(), Some(&ok)).is_empty());
        let bad_sym = ModuleChallenge { prompt: "p".into(), sym: "q".into(), target: 1.0, tolerance: 0.0 };
        assert!(!module_violations("visualAid", &hooke(), Some(&bad_sym)).is_empty());
    }

    #[test]
    fn function_plot_needs_a_mostly_finite_domain() {
        let ok = InteractiveModuleSpec::FunctionPlot { expr: "a * x^2".into(), domain: [-3.0, 3.0], params: vec![var("a", -2.0, 2.0, 1.0)], tangent: true };
        assert!(module_violations("v", &ok, None).is_empty());
        let all_bad = InteractiveModuleSpec::FunctionPlot { expr: "sqrt(x)".into(), domain: [-5.0, -1.0], params: vec![], tangent: false };
        assert!(module_violations("v", &all_bad, None).iter().any(|m| m.contains("solo es finita")));
    }

    #[test]
    fn particle_gradient_and_timeline_rules() {
        let flat = InteractiveModuleSpec::ParticleGradient { high_label: "Fuera".into(), low_label: "Dentro".into(), high_count: 10, low_count: 10, has_barrier: true, channels: 0 };
        let v = module_violations("v", &flat, None);
        assert!(v.iter().any(|m| m.contains("no hay gradiente")) && v.iter().any(|m| m.contains("al menos 1 canal")), "{v:?}");
        let tl = InteractiveModuleSpec::Timeline { events: vec![TimelineEvent { label: "a".into(), detail: "b".into() }] };
        assert!(!module_violations("v", &tl, None).is_empty());
    }
}
