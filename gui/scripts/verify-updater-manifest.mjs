import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { normalizeSignatureText, validateAssetUrl } from "./generate-updater-manifest.mjs";

export function verifyManifest({ text, mode, version, assetUrl, stagingEndpoint }) {
  let manifest;
  try { manifest = JSON.parse(text); } catch { throw new Error("manifest is not valid JSON"); }
  if (manifest?.version !== version) throw new Error("manifest version does not match the expected app version");
  if (typeof manifest.notes !== "string" || !manifest.notes.trim()) throw new Error("manifest release notes are missing");
  const platformNames = Object.keys(manifest.platforms ?? {});
  if (platformNames.length !== 1 || platformNames[0] !== "windows-x86_64") throw new Error("manifest must contain only the windows-x86_64 platform");
  const platform = manifest.platforms["windows-x86_64"];
  if (platform.url !== assetUrl) throw new Error("manifest asset URL does not match the expected URL");
  validateAssetUrl({ mode, version, assetUrl: platform.url, stagingEndpoint });
  if (typeof platform.signature !== "string" || !platform.signature) throw new Error("manifest signature is missing");
  const signatureBytes = Buffer.from(platform.signature, "base64");
  if (signatureBytes.length === 0 || signatureBytes.toString("base64") !== platform.signature) throw new Error("manifest signature is not canonical base64");
  const signatureText = normalizeSignatureText(signatureBytes.toString("utf8"));
  const lines = signatureText.split(/\r?\n/);
  if (lines.length !== 4 || !lines[0].startsWith("untrusted comment: ") || !/^[A-Za-z0-9+/]+={0,2}$/.test(lines[1]) || !lines[2].startsWith("trusted comment: ") || !/^[A-Za-z0-9+/]+={0,2}$/.test(lines[3])) {
    throw new Error("manifest updater signature is malformed");
  }
  return manifest;
}

async function main() {
  const [manifestPath, mode, version, assetUrl, stagingEndpoint] = process.argv.slice(2);
  if (!manifestPath || !mode || !version || !assetUrl) {
    throw new Error("usage: node verify-updater-manifest.mjs <manifest> <stable|staging> <version> <asset-url> [staging-endpoint]");
  }
  const text = await readFile(resolve(manifestPath), "utf8");
  verifyManifest({ text, mode, version, assetUrl, stagingEndpoint });
  console.log(`Updater manifest validated for ${mode} v${version}.`);
}

if (process.argv[1] && resolve(process.argv[1]).toLowerCase() === resolve(import.meta.filename).toLowerCase()) {
  main().catch((error) => {
    console.error(`Updater manifest verification failed: ${error.message}`);
    process.exitCode = 1;
  });
}
