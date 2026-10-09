// Cuts a release: bumps the version everywhere, commits, tags.
//
//   bun run release patch            # 0.1.0 -> 0.1.1   (also: minor | major | X.Y.Z)
//   bun run release minor --push     # ...and push the branch + tag (publishes the release)
//   bun run release patch --dry-run  # show what would happen, change nothing
//
// Pushing the `vX.Y.Z` tag is what triggers .github/workflows/release.yml, which
// builds every platform and publishes the GitHub Release + the updater manifest.
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { loadVersionFiles } from "./check-version";
import { nextVersion, readVersions, versionProblems, writeVersions } from "./version";

const args = process.argv.slice(2);
const flags = new Set(args.filter((a) => a.startsWith("--")));
const target = args.find((a) => !a.startsWith("--"));
const dryRun = flags.has("--dry-run");
const push = flags.has("--push");

function git(...a: string[]): string {
  return execFileSync("git", a, { encoding: "utf8" }).trim();
}

function fail(msg: string): never {
  console.error(`error: ${msg}`);
  process.exit(1);
}

if (!target) fail("indica la versión: patch | minor | major | X.Y.Z   (ej. bun run release patch)");

const files = loadVersionFiles();
const drift = versionProblems(files);
if (drift.length > 0) fail(`${drift.join("; ")} — corrígelo antes de publicar`);

const current = readVersions(files).tauriConf;
let next: string;
try {
  next = nextVersion(current, target);
} catch (e) {
  fail((e as Error).message);
}
const tag = `v${next}`;

if (git("status", "--porcelain") !== "") fail("el árbol de trabajo tiene cambios sin confirmar: haz commit o stash primero");
const branch = git("rev-parse", "--abbrev-ref", "HEAD");
if (branch === "HEAD") fail("estás en un HEAD desacoplado: cambia a una rama");
if (git("tag", "--list", tag) !== "") fail(`el tag ${tag} ya existe`);

console.log(`${current} -> ${next}  (tag ${tag}, rama ${branch})${dryRun ? "  [dry-run]" : ""}`);
if (dryRun) process.exit(0);

const out = writeVersions(files, next);
writeFileSync("package.json", out.packageJson);
writeFileSync("src-tauri/tauri.conf.json", out.tauriConf);
writeFileSync("src-tauri/Cargo.toml", out.cargoToml);
writeFileSync("src-tauri/Cargo.lock", out.cargoLock);

// Cargo.lock was edited by hand: let cargo prove it is still consistent
// (--locked fails if it would need to change it).
try {
  execFileSync("cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1", "--manifest-path", "src-tauri/Cargo.toml"], {
    stdio: "ignore",
  });
} catch {
  git("checkout", "--", "package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock");
  fail("Cargo.lock quedó inconsistente tras subir la versión; se revirtieron los archivos. Corre `cargo update -p learnkit` y reintenta");
}

git("add", "package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock");
git("commit", "-m", `chore(release): ${tag}`);
git("tag", "-a", tag, "-m", `LearnKit ${tag}`);
console.log(`commit y tag ${tag} creados.`);

if (push) {
  git("push", "origin", "HEAD");
  git("push", "origin", tag);
  console.log(`publicado: GitHub Actions construirá y publicará ${tag}.`);
} else {
  console.log(`para publicar: git push origin HEAD && git push origin ${tag}`);
}
