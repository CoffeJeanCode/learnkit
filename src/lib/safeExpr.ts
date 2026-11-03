/**
 * Tiny arithmetic evaluator for the interactive visual modules. Nothing is
 * ever `eval`-ed: only the grammar below is accepted. It MIRRORS
 * `src-tauri/src/domain/safe_expr.rs` (the backend rejects broken formulas at
 * generation time with that twin) — keep both in sync.
 *
 *   expr   = term (("+" | "-") term)*
 *   term   = unary (("*" | "/") unary)*
 *   unary  = ("-" | "+") unary | power
 *   power  = atom ("^" unary)?
 *   atom   = number | ident | ident "(" expr ("," expr)* ")" | "(" expr ")"
 *
 * Implicit multiplication is allowed between a number/`)`/symbol and a
 * number-less value start (`2x`, `3(x+1)`).
 */

export const MAX_EXPR_CHARS = 200;
const MAX_DEPTH = 32;

const FUNCTIONS: Record<string, { arity: number; fn: (a: number, b: number) => number }> = {
  sin: { arity: 1, fn: (a) => Math.sin(a) },
  cos: { arity: 1, fn: (a) => Math.cos(a) },
  tan: { arity: 1, fn: (a) => Math.tan(a) },
  asin: { arity: 1, fn: (a) => Math.asin(a) },
  acos: { arity: 1, fn: (a) => Math.acos(a) },
  atan: { arity: 1, fn: (a) => Math.atan(a) },
  sqrt: { arity: 1, fn: (a) => Math.sqrt(a) },
  abs: { arity: 1, fn: (a) => Math.abs(a) },
  ln: { arity: 1, fn: (a) => Math.log(a) },
  log: { arity: 1, fn: (a) => Math.log10(a) },
  exp: { arity: 1, fn: (a) => Math.exp(a) },
  floor: { arity: 1, fn: (a) => Math.floor(a) },
  ceil: { arity: 1, fn: (a) => Math.ceil(a) },
  min: { arity: 2, fn: (a, b) => Math.min(a, b) },
  max: { arity: 2, fn: (a, b) => Math.max(a, b) },
};

const CONSTANTS: Record<string, number> = { pi: Math.PI, e: Math.E };

type Node =
  | { t: "num"; v: number }
  | { t: "var"; name: string }
  | { t: "neg"; a: Node }
  | { t: "bin"; op: string; a: Node; b: Node }
  | { t: "call"; name: string; args: Node[] };

type Tok =
  | { k: "num"; v: number }
  | { k: "id"; v: string }
  | { k: "op"; v: string }
  | { k: "lp" }
  | { k: "rp" }
  | { k: "comma" };

const own = (o: object, k: string): boolean => Object.prototype.hasOwnProperty.call(o, k);
const isDigit = (c: string) => c >= "0" && c <= "9";
const isAlpha = (c: string) => (c >= "a" && c <= "z") || (c >= "A" && c <= "Z") || c === "_";

function tokenize(src: string): Tok[] {
  const toks: Tok[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/\s/.test(c)) {
      i++;
    } else if (isDigit(c) || c === ".") {
      const start = i;
      while (i < src.length && (isDigit(src[i]) || src[i] === ".")) i++;
      const text = src.slice(start, i);
      const v = Number(text);
      if (!Number.isFinite(v) || text === "." || (text.match(/\./g) ?? []).length > 1) throw new Error(`número inválido "${text}"`);
      toks.push({ k: "num", v });
    } else if (isAlpha(c)) {
      const start = i;
      while (i < src.length && (isAlpha(src[i]) || isDigit(src[i]))) i++;
      toks.push({ k: "id", v: src.slice(start, i) });
    } else {
      if ("+-*/^".includes(c)) toks.push({ k: "op", v: c });
      else if (c === "×" || c === "·") toks.push({ k: "op", v: "*" });
      else if (c === "(") toks.push({ k: "lp" });
      else if (c === ")") toks.push({ k: "rp" });
      else if (c === ",") toks.push({ k: "comma" });
      else throw new Error(`carácter no permitido "${c}"`);
      i++;
    }
  }
  const out: Tok[] = [];
  toks.forEach((t, idx) => {
    if (idx > 0) {
      const prev = toks[idx - 1];
      const prevEndsValue = prev.k === "num" || prev.k === "rp" || (prev.k === "id" && !own(FUNCTIONS, prev.v));
      const nextStartsValue = t.k === "num" || t.k === "id" || t.k === "lp";
      // `name(` is a call: a known function, or an error for an unknown one
      // (never a silent `name * (...)`).
      const call = prev.k === "id" && t.k === "lp";
      if (prevEndsValue && nextStartsValue && !call && !(prev.k === "id" && t.k === "num")) out.push({ k: "op", v: "*" });
    }
    out.push(t);
  });
  return out;
}

class Parser {
  pos = 0;
  constructor(private toks: Tok[]) {}

  private peek(): Tok | undefined {
    return this.toks[this.pos];
  }

  done(): boolean {
    return this.pos >= this.toks.length;
  }

  expr(depth: number): Node {
    if (depth > MAX_DEPTH) throw new Error("expresión demasiado anidada");
    let left = this.term(depth);
    for (let t = this.peek(); t?.k === "op" && (t.v === "+" || t.v === "-"); t = this.peek()) {
      this.pos++;
      left = { t: "bin", op: t.v, a: left, b: this.term(depth) };
    }
    return left;
  }

  private term(depth: number): Node {
    let left = this.unary(depth);
    for (let t = this.peek(); t?.k === "op" && (t.v === "*" || t.v === "/"); t = this.peek()) {
      this.pos++;
      left = { t: "bin", op: t.v, a: left, b: this.unary(depth) };
    }
    return left;
  }

  private unary(depth: number): Node {
    const t = this.peek();
    if (t?.k === "op" && t.v === "-") {
      this.pos++;
      return { t: "neg", a: this.unary(depth + 1) };
    }
    if (t?.k === "op" && t.v === "+") {
      this.pos++;
      return this.unary(depth + 1);
    }
    return this.power(depth);
  }

  private power(depth: number): Node {
    const base = this.atom(depth);
    const t = this.peek();
    if (t?.k === "op" && t.v === "^") {
      this.pos++;
      return { t: "bin", op: "^", a: base, b: this.unary(depth + 1) };
    }
    return base;
  }

  private atom(depth: number): Node {
    const t = this.toks[this.pos++];
    if (!t) throw new Error("la expresión termina antes de tiempo");
    if (t.k === "num") return { t: "num", v: t.v };
    if (t.k === "lp") {
      const inner = this.expr(depth + 1);
      if (this.toks[this.pos++]?.k !== "rp") throw new Error("falta cerrar un paréntesis");
      return inner;
    }
    if (t.k === "id") {
      if (this.peek()?.k === "lp") {
        const spec = own(FUNCTIONS, t.v) ? FUNCTIONS[t.v] : undefined;
        if (!spec) throw new Error(`función desconocida "${t.v}"`);
        this.pos++;
        const args = [this.expr(depth + 1)];
        while (this.peek()?.k === "comma") {
          this.pos++;
          args.push(this.expr(depth + 1));
        }
        if (this.toks[this.pos++]?.k !== "rp") throw new Error("falta cerrar un paréntesis");
        if (args.length !== spec.arity) throw new Error(`${t.v}() espera ${spec.arity} argumento(s), recibió ${args.length}`);
        return { t: "call", name: t.v, args };
      }
      return { t: "var", name: t.v };
    }
    throw new Error("símbolo inesperado");
  }
}

export interface CompiledExpr {
  /** Free symbols (constants `pi`/`e` excluded). */
  symbols: string[];
  /** Evaluates with the given values; throws on a missing symbol. */
  eval: (env: Record<string, number>) => number;
}

/** Parses `src`; throws `Error` (Spanish message) outside the grammar. */
export function compileExpr(src: string): CompiledExpr {
  const text = src.trim();
  if (!text) throw new Error("expresión vacía");
  if (text.length > MAX_EXPR_CHARS) throw new Error(`expresión de más de ${MAX_EXPR_CHARS} caracteres`);
  const parser = new Parser(tokenize(text));
  const root = parser.expr(0);
  if (!parser.done()) throw new Error("sobran símbolos al final de la expresión (¿un paréntesis de más o un `=`?)");

  const symbols = new Set<string>();
  const collect = (n: Node): void => {
    if (n.t === "var") {
      if (!own(CONSTANTS, n.name)) symbols.add(n.name);
    } else if (n.t === "neg") collect(n.a);
    else if (n.t === "bin") {
      collect(n.a);
      collect(n.b);
    } else if (n.t === "call") n.args.forEach(collect);
  };
  collect(root);

  const go = (n: Node, env: Record<string, number>): number => {
    switch (n.t) {
      case "num":
        return n.v;
      case "var": {
        if (own(env, n.name)) return env[n.name];
        if (own(CONSTANTS, n.name)) return CONSTANTS[n.name];
        throw new Error(`símbolo sin valor "${n.name}"`);
      }
      case "neg":
        return -go(n.a, env);
      case "bin": {
        const x = go(n.a, env);
        const y = go(n.b, env);
        return n.op === "+" ? x + y : n.op === "-" ? x - y : n.op === "*" ? x * y : n.op === "/" ? x / y : Math.pow(x, y);
      }
      case "call":
        return FUNCTIONS[n.name].fn(go(n.args[0], env), n.args[1] ? go(n.args[1], env) : 0);
    }
  };

  return { symbols: [...symbols].sort(), eval: (env) => go(root, env) };
}

/** `compileExpr` that returns `null` instead of throwing (for render paths). */
export function tryCompileExpr(src: string): CompiledExpr | null {
  try {
    return compileExpr(src);
  } catch {
    return null;
  }
}
