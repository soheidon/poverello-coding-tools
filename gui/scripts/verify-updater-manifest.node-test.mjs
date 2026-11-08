import test from "node:test";
import assert from "node:assert/strict";
import { verifyManifest } from "./verify-updater-manifest.mjs";

const version = "0.27.1";
const endpoint = "https://updates-staging.example.net/pct/latest.json";
const assetUrl = "https://updates-staging.example.net/pct/Poverello-Coding-Tools_0.27.1_x64-setup-staging.exe";
const signatureText = "untrusted comment: signature from test key\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\ntrusted comment: timestamp:1\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const manifest = {
  version,
  notes: "Staging release notes",
  platforms: {
    "windows-x86_64": {
      url: assetUrl,
      signature: Buffer.from(signatureText).toString("base64"),
    },
  },
};

test("accepts a staging manifest with the expected version, endpoint, asset, and signature shape", () => {
  assert.equal(verifyManifest({ text: JSON.stringify(manifest), mode: "staging", version, assetUrl, stagingEndpoint: endpoint }).version, version);
});

test("accepts a GitHub staging smoke prerelease manifest under INS-007", () => {
  const smokeEndpoint = "https://github.com/soheidon/poverello-coding-tools/releases/download/pct-updater-smoke-v0.27.1-test/latest.json";
  const smokeAssetUrl = "https://github.com/soheidon/poverello-coding-tools/releases/download/pct-updater-smoke-v0.27.1-test/Poverello-Coding-Tools_0.27.1_x64-setup-staging.exe";
  const smokeManifest = {
    ...manifest,
    platforms: {
      "windows-x86_64": {
        url: smokeAssetUrl,
        signature: manifest.platforms["windows-x86_64"].signature,
      },
    },
  };
  assert.equal(verifyManifest({ text: JSON.stringify(smokeManifest), mode: "staging", version, assetUrl: smokeAssetUrl, stagingEndpoint: smokeEndpoint }).version, version);
});

test("rejects malformed JSON, version, platform, URL, signature, and production-host staging manifest", () => {
  const input = { mode: "staging", version, assetUrl, stagingEndpoint: endpoint };
  assert.throws(() => verifyManifest({ ...input, text: "{" }), /valid JSON/);
  assert.throws(() => verifyManifest({ ...input, text: JSON.stringify({ ...manifest, version: "0.27.0" }) }), /version/);
  assert.throws(() => verifyManifest({ ...input, text: JSON.stringify({ ...manifest, platforms: {} }) }), /windows-x86_64/);
  assert.throws(() => verifyManifest({ ...input, text: JSON.stringify({ ...manifest, platforms: { "windows-x86_64": { ...manifest.platforms["windows-x86_64"], url: "https://other.example/file.exe" } } }) }), /asset URL/);
  assert.throws(() => verifyManifest({ ...input, text: JSON.stringify({ ...manifest, platforms: { "windows-x86_64": { ...manifest.platforms["windows-x86_64"], signature: "%%%" } } }) }), /canonical base64/);
  assert.throws(() => verifyManifest({ ...input, stagingEndpoint: "https://github.com/soheidon/test/latest.json", text: JSON.stringify(manifest) }), /GitHub production/);
});
