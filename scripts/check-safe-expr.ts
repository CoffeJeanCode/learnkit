// Asserts the formula evaluator behind the interactive modules: same grammar as
// `src-tauri/src/domain/safe_expr.rs`, never `eval`s, rejects anything outside
// the grammar. Run with: bun scripts/check-safe-expr.ts
import assert from "node:assert/strict";
import { compileExpr, tryCompileExpr } from "../src/lib/safeExpr";

const ev = (src: string, env: Record<string, number> = {}) => compileExpr(src).eval(env);

assert.equal(ev("1 + 2 * 3"), 7);
assert.equal(ev("(1 + 2) * 3"), 9);
assert.equal(ev("2 ^ 3 ^ 2"), 512, "power is right associative");
assert.equal(ev("-x ^ 2", { x: 3 }), -9, "unary minus binds looser than ^");
assert.equal(ev("10 / 4"), 2.5);
assert.equal(ev("2x", { x: 5 }), 10, "implicit multiplication");
assert.equal(ev("3(x + 1)", { x: 1 }), 6);
assert.ok(Math.abs(ev("sin(pi / 2)") - 1) < 1e-12);
assert.equal(ev("max(a, b) + min(a, b)", { a: 2, b: 7 }), 9);
assert.equal(ev("sqrt(x^2 + y^2)", { x: 3, y: 4 }), 5);
assert.deepEqual(compileExpr("m * g * h + sin(pi) * k").symbols, ["g", "h", "k", "m"]);

for (const bad of ["", "1 +", "(1 + 2", "1 + 2)", "foo(1)", "sin(1, 2)", "x = 2", "x; y", "a.b", "a[0]", "1 2 3 )", "x => x", "`x`", "{}"]) {
  assert.equal(tryCompileExpr(bad), null, `should reject ${JSON.stringify(bad)}`);
}
assert.equal(tryCompileExpr("1+".repeat(200)), null, "too long");
assert.equal(tryCompileExpr(`${"(".repeat(100)}1${")".repeat(100)}`), null, "too deep");

// Prototype names are plain (missing) symbols, never reached through the object chain.
assert.throws(() => compileExpr("constructor * 2").eval({}), /sin valor/);
assert.throws(() => compileExpr("toString + 1").eval({}), /sin valor/);
assert.deepEqual(compileExpr("constructor").symbols, ["constructor"]);
assert.throws(() => compileExpr("k * x").eval({ k: 2 }), /sin valor/, "a missing symbol is an error, not 0");

console.log("safe expr: OK");
