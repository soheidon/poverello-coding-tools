import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createManifest, extractReleaseNotes } from "./generate-updater-manifest.mjs";

const installerBytes = Buffer.from("fixture installer payload");
const sha256 = createHash("sha256").update(installerBytes).digest("hex");
const valid = {
  version: "0.27.0",
  installerBytes,
  signature: "untrusted comment: signature from test key\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\ntrusted comment: timestamp:1\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
  expectedSha256: sha256,
  notes: "Release notes from NEWS.md",
  assetUrl: "https://github.com/soheidon/poverello-coding-tools/releases/download/v0.27.0/Poverello-Coding-Tools_0.27.0_x64-setup.exe",
};

test("creates stable Tauri static manifest with installer signature and release notes", () => {
  const manifest = createManifest(valid);
  assert.deepEqual(Object.keys(manifest.platforms), ["windows-x86_64"]);
  assert.equal(manifest.version, valid.version);
  assert.equal(manifest.notes, valid.notes);
  assert.equal(Buffer.from(manifest.platforms["windows-x86_64"].signature, "base64").toString("utf8"), valid.signature);
  assert.equal(manifest.platforms["windows-x86_64"].url, valid.assetUrl);
  assert.deepEqual(createManifest(valid), manifest);
});

test("rejects installer tampering and malformed signatures", () => {
  assert.throws(() => createManifest({ ...valid, installerBytes: Buffer.from("tampered") }), /SHA-256/);
  assert.throws(() => createManifest({ ...valid, signature: "" }), /signature/);
  assert.throws(() => createManifest({ ...valid, version: "0.27" }), /semantic version/);
});

test("rejects non-release URLs and empty NEWS notes", () => {
  assert.throws(() => createManifest({ ...valid, assetUrl: "http://example.com/installer.exe" }), /HTTPS GitHub release/);
  assert.throws(() => createManifest({ ...valid, notes: "  " }), /release notes/);
});

test("creates a staging manifest only for the configured HTTPS endpoint and versioned staging asset", () => {
  const staging = {
    ...valid,
    mode: "staging",
    version: "0.27.1",
    stagingEndpoint: "https://updates-staging.example.net/pct/latest.json",
    assetUrl: "https://updates-staging.example.net/pct/Poverello-Coding-Tools_0.27.1_x64-setup-staging.exe",
  };
  const manifest = createManifest(staging);
  assert.equal(manifest.version, "0.27.1");
  assert.equal(manifest.platforms["windows-x86_64"].url, staging.assetUrl);
  assert.throws(() => createManifest({ ...staging, assetUrl: valid.assetUrl }), /configured endpoint/);
  assert.throws(() => createManifest({ ...staging, stagingEndpoint: "http://updates-staging.example.net/pct/latest.json" }), /HTTPS/);
  assert.throws(() => createManifest({ ...staging, stagingEndpoint: "https://github.com/test/release/latest.json" }), /GitHub production/);
  assert.throws(() => createManifest({ ...staging, assetUrl: "https://updates-staging.example.net/pct/../Poverello-Coding-Tools_0.27.1_x64-setup-staging.exe" }), /path traversal/);
});

test("supports GitHub staging smoke prerelease endpoint under INS-007 and enforces isolation", () => {
  const smoke = {
    ...valid,
    mode: "staging",
    version: "0.27.1",
    stagingEndpoint: "https://github.com/soheidon/poverello-coding-tools/releases/download/pct-updater-smoke-v0.27.1-run1/latest.json",
    assetUrl: "https://github.com/soheidon/poverello-coding-tools/releases/download/pct-updater-smoke-v0.27.1-run1/Poverello-Coding-Tools_0.27.1_x64-setup-staging.exe",
  };
  const manifest = createManifest(smoke);
  assert.equal(manifest.version, "0.27.1");
  assert.equal(manifest.platforms["windows-x86_64"].url, smoke.assetUrl);

  // Rejects production latest endpoint
  assert.throws(() => createManifest({ ...smoke, stagingEndpoint: "https://github.com/soheidon/poverello-coding-tools/releases/latest/download/latest.json" }), /GitHub production/);
  // Rejects release tag without pct-updater-smoke- prefix or with uppercase prefix
  assert.throws(() => createManifest({ ...smoke, stagingEndpoint: "https://github.com/soheidon/poverello-coding-tools/releases/download/v0.27.1/latest.json" }), /GitHub production/);
  assert.throws(() => createManifest({ ...smoke, stagingEndpoint: "https://github.com/soheidon/poverello-coding-tools/releases/download/PCT-UPDATER-SMOKE-v0.27.1/latest.json" }), /GitHub production/);
  // Rejects different repository
  assert.throws(() => createManifest({ ...smoke, stagingEndpoint: "https://github.com/other-org/poverello-coding-tools/releases/download/pct-updater-smoke-v0.27.1-run1/latest.json" }), /GitHub production/);
  // Rejects tag mismatch between endpoint and asset
  assert.throws(() => createManifest({ ...smoke, assetUrl: "https://github.com/soheidon/poverello-coding-tools/releases/download/pct-updater-smoke-other/Poverello-Coding-Tools_0.27.1_x64-setup-staging.exe" }), /staging asset URL must match/);
  // Rejects production installer filename without -staging suffix
  assert.throws(() => createManifest({ ...smoke, assetUrl: "https://github.com/soheidon/poverello-coding-tools/releases/download/pct-updater-smoke-v0.27.1-run1/Poverello-Coding-Tools_0.27.1_x64-setup.exe" }), /staging asset URL must match/);
});

test("does not permit a staging URL in stable mode or an unknown mode", () => {
  const stagingUrl = "https://updates-staging.example.net/pct/Poverello-Coding-Tools_0.27.0_x64-setup-staging.exe";
  assert.throws(() => createManifest({ ...valid, assetUrl: stagingUrl }), /fixed HTTPS GitHub release/);
  assert.throws(() => createManifest({ ...valid, mode: "preview" }), /mode must be stable or staging/);
});

test("extracts only the selected version's NEWS section", () => {
  const news = "# News\n## v0.27.0 — Today\n\nFirst release note.\n## v0.26.2 — Earlier\n\nOld note.";
  assert.equal(extractReleaseNotes(news, "0.27.0"), "First release note.");
  assert.equal(extractReleaseNotes(news, "0.28.0"), null);
});
