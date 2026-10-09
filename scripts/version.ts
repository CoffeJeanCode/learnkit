// Pure helpers for the release version: one number lives in four files that
// must never drift (the updater compares `tauri.conf.json`'s version with the
// release's `latest.json`, and the tag must name that same number).
// No I/O here — `release.ts` and `check-version.ts` read/write the files.

export type Bump = "patch" | "minor" | "major";

export interface Semver {
  major: number;
  minor: number;
  patch: number;
}

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

export function parseVersion(v: string): Semver {
  const m = SEMVER.exec(v.trim());
  if (!m) throw new Error(`versión inválida "${v}" (se espera X.Y.Z, sin prefijo "v" ni sufijos)`);
  return { major: Number(m[1]), minor: Number(m[2]), patch: Number(m[3]) };
}

export function formatVersion(s: Semver): string {
  return `${s.major}.${s.minor}.${s.patch}`;
}

export function compareVersions(a: string, b: string): number {
  const x = parseVersion(a);
  const y = parseVersion(b);
  return x.major - y.major || x.minor - y.minor || x.patch - y.patch;
}

/** `target` is a bump keyword or an explicit X.Y.Z; the result must be newer than `current`. */
export function nextVersion(current: string, target: string): string {
  const cur = parseVersion(current);
  let next: string;
  if (target === "major") next = formatVersion({ major: cur.major + 1, minor: 0, patch: 0 });
  else if (target === "minor") next = formatVersion({ major: cur.major, minor: cur.minor + 1, patch: 0 });
  else if (target === "patch") next = formatVersion({ ...cur, patch: cur.patch + 1 });
  else next = formatVersion(parseVersion(target.replace(/^v/, "")));
  if (compareVersions(next, current) <= 0) throw new Error(`la versión nueva (${next}) debe ser mayor que la actual (${current})`);
  return next;
}

/** The four places the version lives, as text. */
export interface VersionFiles {
  packageJson: string;
  tauriConf: string;
  cargoToml: string;
  cargoLock: string;
}

const JSON_VERSION = /("version"\s*:\s*")(\d+\.\d+\.\d+)(")/;
// Line endings: on Windows runners git checks files out with CRLF, so every
// pattern spanning lines accepts `\r?\n` (and replaces only the number, so the
// file keeps whatever endings it had).
const CARGO_TOML_VERSION = /(\[package\][^\[]*?\r?\nversion\s*=\s*")(\d+\.\d+\.\d+)(")/;
const CARGO_LOCK_VERSION = /(\[\[package\]\]\r?\nname = "learnkit"\r?\nversion = ")(\d+\.\d+\.\d+)(")/;

function read(text: string, re: RegExp, label: string): string {
  const m = re.exec(text);
  if (!m) throw new Error(`no se encontró la versión en ${label}`);
  return m[2];
}

export function readVersions(f: VersionFiles): Record<keyof VersionFiles, string> {
  return {
    packageJson: read(f.packageJson, JSON_VERSION, "package.json"),
    tauriConf: read(f.tauriConf, JSON_VERSION, "src-tauri/tauri.conf.json"),
    cargoToml: read(f.cargoToml, CARGO_TOML_VERSION, "src-tauri/Cargo.toml"),
    cargoLock: read(f.cargoLock, CARGO_LOCK_VERSION, "src-tauri/Cargo.lock"),
  };
}

/** Returns the files with the version replaced (formatting untouched). */
export function writeVersions(f: VersionFiles, version: string): VersionFiles {
  parseVersion(version);
  const sub = (text: string, re: RegExp, label: string) => {
    read(text, re, label);
    return text.replace(re, `$1${version}$3`);
  };
  return {
    packageJson: sub(f.packageJson, JSON_VERSION, "package.json"),
    tauriConf: sub(f.tauriConf, JSON_VERSION, "src-tauri/tauri.conf.json"),
    cargoToml: sub(f.cargoToml, CARGO_TOML_VERSION, "src-tauri/Cargo.toml"),
    cargoLock: sub(f.cargoLock, CARGO_LOCK_VERSION, "src-tauri/Cargo.lock"),
  };
}

/** Empty when all four agree (and match `tag`, if given); otherwise the problems. */
export function versionProblems(f: VersionFiles, tag?: string): string[] {
  const v = readVersions(f);
  const distinct = new Set(Object.values(v));
  const problems: string[] = [];
  if (distinct.size > 1) {
    problems.push(`las versiones no coinciden: ${Object.entries(v).map(([k, x]) => `${k}=${x}`).join(", ")}`);
  }
  if (tag) {
    const tagVersion = tag.replace(/^refs\/tags\//, "").replace(/^v/, "");
    if (!distinct.has(tagVersion) || distinct.size > 1) {
      problems.push(`el tag "${tag}" no coincide con la versión de los archivos (${[...distinct].join(", ")})`);
    }
  }
  return problems;
}
