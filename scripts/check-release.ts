// Asserts the pure release-version logic. Run with: bun scripts/check-release.ts
import assert from "node:assert/strict";
import { compareVersions, nextVersion, parseVersion, readVersions, versionProblems, writeVersions, type VersionFiles } from "./version";

// --- bumping ---------------------------------------------------------------
assert.equal(nextVersion("0.1.0", "patch"), "0.1.1");
assert.equal(nextVersion("0.1.9", "minor"), "0.2.0");
assert.equal(nextVersion("1.4.7", "major"), "2.0.0");
assert.equal(nextVersion("0.1.0", "0.3.2"), "0.3.2");
assert.equal(nextVersion("0.1.0", "v0.3.2"), "0.3.2", "an explicit tag-style version is accepted");
assert.throws(() => nextVersion("0.2.0", "0.2.0"), /mayor/);
assert.throws(() => nextVersion("0.2.0", "0.1.9"), /mayor/);
assert.throws(() => nextVersion("0.2.0", "1.0"), /inválida/);
assert.throws(() => parseVersion("1.0.0-beta.1"), /inválida/);
assert.ok(compareVersions("0.10.0", "0.9.0") > 0, "compared numerically, not as text");

// --- rewriting the four files ---------------------------------------------
const files: VersionFiles = {
  packageJson: '{\n  "name": "learnkit",\n  "version": "0.1.0",\n  "private": true\n}\n',
  tauriConf: '{\n  "productName": "LearnKit",\n  "version": "0.1.0",\n  "identifier": "x"\n}\n',
  cargoToml: '[package]\nname = "learnkit"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\nserde = { version = "1" }\n',
  cargoLock: '[[package]]\nname = "other"\nversion = "9.9.9"\n\n[[package]]\nname = "learnkit"\nversion = "0.1.0"\ndependencies = []\n',
};
assert.deepEqual(Object.values(readVersions(files)), ["0.1.0", "0.1.0", "0.1.0", "0.1.0"]);

const bumped = writeVersions(files, "0.2.0");
assert.deepEqual(Object.values(readVersions(bumped)), ["0.2.0", "0.2.0", "0.2.0", "0.2.0"]);
assert.ok(bumped.cargoLock.includes('name = "other"\nversion = "9.9.9"'), "other packages keep their version");
assert.ok(bumped.cargoToml.includes('serde = { version = "1" }'), "dependency versions are untouched");
assert.equal(bumped.packageJson, files.packageJson.replace("0.1.0", "0.2.0"), "only the number changes");

// --- consistency / tag check ----------------------------------------------
assert.deepEqual(versionProblems(files), []);
assert.deepEqual(versionProblems(files, "v0.1.0"), []);
assert.deepEqual(versionProblems(files, "refs/tags/v0.1.0"), []);
assert.equal(versionProblems(files, "v0.2.0").length, 1, "tag newer than the files");
assert.equal(versionProblems({ ...files, cargoToml: files.cargoToml.replace("0.1.0", "0.1.1") }).length, 1, "drift between files");
assert.throws(() => writeVersions({ ...files, cargoLock: "" }, "0.2.0"), /Cargo.lock/);

// --- Windows checkouts use CRLF line endings -------------------------------
const crlf = (f: VersionFiles): VersionFiles => ({
  packageJson: f.packageJson.replace(/\n/g, "\r\n"),
  tauriConf: f.tauriConf.replace(/\n/g, "\r\n"),
  cargoToml: f.cargoToml.replace(/\n/g, "\r\n"),
  cargoLock: f.cargoLock.replace(/\n/g, "\r\n"),
});
const win = crlf(files);
assert.deepEqual(Object.values(readVersions(win)), ["0.1.0", "0.1.0", "0.1.0", "0.1.0"], "CRLF files are read");
assert.deepEqual(versionProblems(win, "v0.1.0"), []);
const winBumped = writeVersions(win, "0.2.0");
assert.deepEqual(Object.values(readVersions(winBumped)), ["0.2.0", "0.2.0", "0.2.0", "0.2.0"]);
assert.equal(winBumped.cargoLock, win.cargoLock.replace("0.1.0", "0.2.0"), "CRLF endings are preserved, only the number changes");
assert.equal(winBumped.cargoToml, win.cargoToml.replace("0.1.0", "0.2.0"));
// Mixed endings (a repo with autocrlf converting only some files) also work.
assert.deepEqual(versionProblems({ ...files, cargoLock: win.cargoLock }, "v0.1.0"), []);

console.log("release version logic: OK");
