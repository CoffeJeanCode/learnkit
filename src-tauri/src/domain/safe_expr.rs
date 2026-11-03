//! Tiny, dependency-free arithmetic expression evaluator for the interactive
//! visual modules (`StaticVisualSpec::InteractiveModule`).
//!
//! The model writes formulas such as `k * x` or `m * g * h`; the student's
//! browser evaluates them as sliders move. Nothing is ever `eval`-ed: the
//! grammar below is the ONLY thing accepted, and `src/lib/safeExpr.ts` mirrors
//! it exactly (keep both in sync). The backend uses this module to REJECT a
//! broken formula at generation time (unknown symbol, unbalanced parenthesis,
//! non-finite result) so the student never sees a dead widget.
//!
//! Grammar (implicit multiplication allowed between a number and a symbol or
//! `(`, e.g. `2x`, `3(x+1)`):
//! ```text
//! expr   = term (("+" | "-") term)*
//! term   = unary (("*" | "/") unary)*
//! unary  = ("-" | "+") unary | power
//! power  = atom ("^" unary)?
//! atom   = number | ident | ident "(" expr ("," expr)* ")" | "(" expr ")"
//! ```

use std::collections::{BTreeSet, HashMap};

pub const MAX_EXPR_CHARS: usize = 200;
const MAX_DEPTH: usize = 32;

/// Functions the grammar accepts, with their arity.
const FUNCTIONS: &[(&str, usize)] = &[
    ("sin", 1),
    ("cos", 1),
    ("tan", 1),
    ("asin", 1),
    ("acos", 1),
    ("atan", 1),
    ("sqrt", 1),
    ("abs", 1),
    ("ln", 1),
    ("log", 1),
    ("exp", 1),
    ("floor", 1),
    ("ceil", 1),
    ("min", 2),
    ("max", 2),
];

/// Constants resolved without an environment entry.
const CONSTANTS: &[(&str, f64)] = &[("pi", std::f64::consts::PI), ("e", std::f64::consts::E)];

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(f64),
    Var(String),
    Neg(Box<Node>),
    Bin(char, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Comma,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    root: Node,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let n = text.parse::<f64>().map_err(|_| format!("número inválido \"{text}\""))?;
            toks.push(Tok::Num(n));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            toks.push(Tok::Ident(chars[start..i].iter().collect()));
        } else {
            match c {
                '+' | '-' | '*' | '/' | '^' => toks.push(Tok::Op(c)),
                '×' | '·' => toks.push(Tok::Op('*')),
                '(' => toks.push(Tok::LParen),
                ')' => toks.push(Tok::RParen),
                ',' => toks.push(Tok::Comma),
                other => return Err(format!("carácter no permitido \"{other}\"")),
            }
            i += 1;
        }
    }
    // Implicit multiplication: `2x`, `2(x+1)`, `)(`, `)x`.
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    for (idx, t) in toks.iter().enumerate() {
        if idx > 0 {
            let prev = &toks[idx - 1];
            let prev_ends_value = matches!(prev, Tok::Num(_) | Tok::RParen) || matches!(prev, Tok::Ident(name) if !is_function(name));
            let next_starts_value = matches!(t, Tok::Num(_) | Tok::Ident(_) | Tok::LParen);
            // `name(` is a call: a known function, or an error for an unknown
            // one (never a silent `name * (...)`).
            let call = matches!(prev, Tok::Ident(_)) && matches!(t, Tok::LParen);
            if prev_ends_value && next_starts_value && !call && !(matches!(prev, Tok::Ident(_)) && matches!(t, Tok::Num(_))) {
                out.push(Tok::Op('*'));
            }
        }
        out.push(t.clone());
    }
    Ok(out)
}

fn is_function(name: &str) -> bool {
    FUNCTIONS.iter().any(|(n, _)| *n == name)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn expr(&mut self, depth: usize) -> Result<Node, String> {
        if depth > MAX_DEPTH {
            return Err("expresión demasiado anidada".into());
        }
        let mut left = self.term(depth)?;
        while let Some(Tok::Op(op @ ('+' | '-'))) = self.peek().cloned() {
            self.pos += 1;
            let right = self.term(depth)?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn term(&mut self, depth: usize) -> Result<Node, String> {
        let mut left = self.unary(depth)?;
        while let Some(Tok::Op(op @ ('*' | '/'))) = self.peek().cloned() {
            self.pos += 1;
            let right = self.unary(depth)?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self, depth: usize) -> Result<Node, String> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.pos += 1;
                Ok(Node::Neg(Box::new(self.unary(depth + 1)?)))
            }
            Some(Tok::Op('+')) => {
                self.pos += 1;
                self.unary(depth + 1)
            }
            _ => self.power(depth),
        }
    }

    fn power(&mut self, depth: usize) -> Result<Node, String> {
        let base = self.atom(depth)?;
        if let Some(Tok::Op('^')) = self.peek() {
            self.pos += 1;
            let exp = self.unary(depth + 1)?;
            return Ok(Node::Bin('^', Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn atom(&mut self, depth: usize) -> Result<Node, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Node::Num(n)),
            Some(Tok::LParen) => {
                let inner = self.expr(depth + 1)?;
                match self.next() {
                    Some(Tok::RParen) => Ok(inner),
                    _ => Err("falta cerrar un paréntesis".into()),
                }
            }
            Some(Tok::Ident(name)) => {
                if let Some(Tok::LParen) = self.peek() {
                    let Some((_, arity)) = FUNCTIONS.iter().find(|(n, _)| *n == name) else {
                        return Err(format!("función desconocida \"{name}\""));
                    };
                    self.pos += 1;
                    let mut args = vec![self.expr(depth + 1)?];
                    while let Some(Tok::Comma) = self.peek() {
                        self.pos += 1;
                        args.push(self.expr(depth + 1)?);
                    }
                    match self.next() {
                        Some(Tok::RParen) => {}
                        _ => return Err("falta cerrar un paréntesis".into()),
                    }
                    if args.len() != *arity {
                        return Err(format!("{name}() espera {arity} argumento(s), recibió {}", args.len()));
                    }
                    Ok(Node::Call(name, args))
                } else {
                    Ok(Node::Var(name))
                }
            }
            Some(other) => Err(format!("símbolo inesperado {other:?}")),
            None => Err("la expresión termina antes de tiempo".into()),
        }
    }
}

impl Expr {
    /// Parses `src`. Rejects anything outside the grammar above.
    pub fn parse(src: &str) -> Result<Expr, String> {
        let src = src.trim();
        if src.is_empty() {
            return Err("expresión vacía".into());
        }
        if src.chars().count() > MAX_EXPR_CHARS {
            return Err(format!("expresión de más de {MAX_EXPR_CHARS} caracteres"));
        }
        let mut parser = Parser { toks: tokenize(src)?, pos: 0 };
        let root = parser.expr(0)?;
        if parser.pos < parser.toks.len() {
            return Err("sobran símbolos al final de la expresión (¿un paréntesis de más o un `=`?)".into());
        }
        Ok(Expr { root })
    }

    /// Free symbols (constants `pi`/`e` excluded).
    pub fn symbols(&self) -> BTreeSet<String> {
        fn walk(n: &Node, out: &mut BTreeSet<String>) {
            match n {
                Node::Num(_) => {}
                Node::Var(name) => {
                    if !CONSTANTS.iter().any(|(c, _)| c == name) {
                        out.insert(name.clone());
                    }
                }
                Node::Neg(a) => walk(a, out),
                Node::Bin(_, a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Node::Call(_, args) => args.iter().for_each(|a| walk(a, out)),
            }
        }
        let mut out = BTreeSet::new();
        walk(&self.root, &mut out);
        out
    }

    pub fn eval(&self, env: &HashMap<String, f64>) -> Result<f64, String> {
        fn go(n: &Node, env: &HashMap<String, f64>) -> Result<f64, String> {
            Ok(match n {
                Node::Num(v) => *v,
                Node::Var(name) => {
                    if let Some(v) = env.get(name) {
                        *v
                    } else if let Some((_, c)) = CONSTANTS.iter().find(|(c, _)| c == name) {
                        *c
                    } else {
                        return Err(format!("símbolo sin valor \"{name}\""));
                    }
                }
                Node::Neg(a) => -go(a, env)?,
                Node::Bin(op, a, b) => {
                    let (x, y) = (go(a, env)?, go(b, env)?);
                    match op {
                        '+' => x + y,
                        '-' => x - y,
                        '*' => x * y,
                        '/' => x / y,
                        _ => x.powf(y),
                    }
                }
                Node::Call(name, args) => {
                    let a = go(&args[0], env)?;
                    match name.as_str() {
                        "sin" => a.sin(),
                        "cos" => a.cos(),
                        "tan" => a.tan(),
                        "asin" => a.asin(),
                        "acos" => a.acos(),
                        "atan" => a.atan(),
                        "sqrt" => a.sqrt(),
                        "abs" => a.abs(),
                        "ln" => a.ln(),
                        "log" => a.log10(),
                        "exp" => a.exp(),
                        "floor" => a.floor(),
                        "ceil" => a.ceil(),
                        "min" => a.min(go(&args[1], env)?),
                        "max" => a.max(go(&args[1], env)?),
                        other => return Err(format!("función desconocida \"{other}\"")),
                    }
                }
            })
        }
        go(&self.root, env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn ev(src: &str, pairs: &[(&str, f64)]) -> f64 {
        Expr::parse(src).unwrap().eval(&env(pairs)).unwrap()
    }

    #[test]
    fn arithmetic_precedence_and_power() {
        assert_eq!(ev("1 + 2 * 3", &[]), 7.0);
        assert_eq!(ev("(1 + 2) * 3", &[]), 9.0);
        assert_eq!(ev("2 ^ 3 ^ 2", &[]), 512.0, "power is right associative");
        assert_eq!(ev("-x ^ 2", &[("x", 3.0)]), -9.0, "unary minus binds looser than ^");
        assert_eq!(ev("10 / 4", &[]), 2.5);
    }

    #[test]
    fn implicit_multiplication_and_functions() {
        assert_eq!(ev("2x", &[("x", 5.0)]), 10.0);
        assert_eq!(ev("3(x + 1)", &[("x", 1.0)]), 6.0);
        assert!((ev("sin(pi / 2)", &[]) - 1.0).abs() < 1e-12);
        assert_eq!(ev("max(a, b) + min(a, b)", &[("a", 2.0), ("b", 7.0)]), 9.0);
        assert_eq!(ev("sqrt(x^2 + y^2)", &[("x", 3.0), ("y", 4.0)]), 5.0);
    }

    #[test]
    fn symbols_exclude_constants_and_functions() {
        let e = Expr::parse("m * g * h + sin(pi) * k").unwrap();
        assert_eq!(e.symbols().into_iter().collect::<Vec<_>>(), vec!["g", "h", "k", "m"]);
    }

    #[test]
    fn rejects_everything_outside_the_grammar() {
        for bad in ["", "1 +", "(1 + 2", "1 + 2)", "foo(1)", "sin(1, 2)", "x = 2", "x; y", "a.b", "__proto__[0]", "1 2 3 )"] {
            assert!(Expr::parse(bad).is_err(), "should reject {bad:?}");
        }
        assert!(Expr::parse(&"1+".repeat(200)).is_err(), "too long");
        assert!(Expr::parse(&format!("{}1{}", "(".repeat(100), ")".repeat(100))).is_err(), "too deep");
    }

    #[test]
    fn missing_symbol_is_an_error_not_a_zero() {
        assert!(Expr::parse("k * x").unwrap().eval(&env(&[("k", 2.0)])).is_err());
    }
}
