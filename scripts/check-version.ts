// Fails when package.json, tauri.conf.json, Cargo.toml and Cargo.lock disagree,
// or when a release tag does not name that version. Used by CI before a release.
//   bun scripts/check-version.ts            # files must agree
//   bun scripts/check-version.ts v0.2.0     # ...and match this tag
import { readFileSync } from "node:fs";
import { versionProblems, type VersionFiles } from "./version";

export function loadVersionFiles(root = "."): VersionFiles {
  const rd = (p: string) => readFileSync(`${root}/${p}`, "utf8");
  return {
    packageJson: rd("package.json"),
    tauriConf: rd("src-tauri/tauri.conf.json"),
    cargoToml: rd("src-tauri/Cargo.toml"),
    cargoLock: rd("src-tauri/Cargo.lock"),
  };
}

if (import.meta.main) {
  const problems = versionProblems(loadVersionFiles(), process.argv[2]);
  if (problems.length > 0) {
    for (const p of problems) console.error(`error: ${p}`);
    process.exit(1);
  }
  console.log("versión consistente en package.json, tauri.conf.json, Cargo.toml y Cargo.lock");
}
