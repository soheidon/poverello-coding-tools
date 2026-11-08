import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const args = Object.fromEntries(process.argv.slice(2).map((arg, index, all) => {
  if (!arg.startsWith("--")) return null;
  return [arg.slice(2), all[index + 1]];
}).filter(Boolean));

function fail(message) {
  console.error(`Updater manifest: ${message}`);
  process.exitCode = 1;
}

export function normalizeSignatureText(signatureText) {
  if (typeof signatureText !== "string") return "";
  const trimmed = signatureText.trim();
  if (trimmed.startsWith("untrusted comment: ")) {
    return trimmed;
  }
  try {
    const decoded = Buffer.from(trimmed, "base64").toString("utf8").trim();
    if (decoded.startsWith("untrusted comment: ")) {
      return decoded;
    }
  } catch { }
  return trimmed;
}

function parseHttpsUrl(value, label) {
  let url;
  try { url = new URL(value); } catch { throw new Error(`${label} must be an absolute HTTPS URL`); }
  if (url.protocol !== "https:" || url.username || url.password || url.search || url.hash) {
    throw new Error(`${label} must be an HTTPS URL without credentials, query, or fragment`);
  }
  if (/%2f|%5c|(?:^|\/)(?:\.|%2e){2}(?:\/|%2f|%5c|$)/i.test(value)) {
    throw new Error(`${label} must not contain path traversal or encoded separators`);
  }
  return url;
}

export function validateAssetUrl({ mode, version, assetUrl, stagingEndpoint }) {
  if (mode === "stable") {
    const escapedVersion = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const expected = new RegExp(`^https://github\\.com/soheidon/poverello-coding-tools/releases/download/v${escapedVersion}/Poverello-Coding-Tools_${escapedVersion}_x64-setup\\.exe$`);
    if (!expected.test(assetUrl ?? "")) throw new Error("stable asset URL must be the fixed HTTPS GitHub release installer URL for this version");
    return;
  }
  if (mode !== "staging") throw new Error("mode must be stable or staging");
  const endpoint = parseHttpsUrl(stagingEndpoint ?? "", "staging endpoint");
  const asset = parseHttpsUrl(assetUrl ?? "", "staging asset URL");
  const isGitHubHost = endpoint.hostname === "github.com" ||
    endpoint.hostname.endsWith(".github.com") ||
    endpoint.hostname === "github.io" ||
    endpoint.hostname.endsWith(".github.io") ||
    endpoint.hostname.endsWith(".githubusercontent.com");

  if (isGitHubHost) {
    const smokeMatch = endpoint.origin === "https://github.com" &&
      /^\/soheidon\/poverello-coding-tools\/releases\/download\/(pct-updater-smoke-[0-9A-Za-z._-]+)\/latest\.json$/.exec(endpoint.pathname);
    if (!smokeMatch) {
      throw new Error("staging endpoint must not use GitHub production hosting");
    }
  }
  if (!endpoint.pathname.endsWith("/latest.json")) throw new Error("staging endpoint path must end in /latest.json");
  const basePath = endpoint.pathname.slice(0, -"latest.json".length);
  const expectedPath = `${basePath}Poverello-Coding-Tools_${version}_x64-setup-staging.exe`;
  if (asset.origin !== endpoint.origin || asset.pathname !== expectedPath) {
    throw new Error("staging asset URL must match the configured endpoint origin and versioned staging path");
  }
}

export function createManifest({ mode = "stable", stagingEndpoint, version, installerBytes, signature, expectedSha256, notes, assetUrl }) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version ?? "")) throw new Error("invalid semantic version");
  if (!Buffer.isBuffer(installerBytes) || installerBytes.length === 0) throw new Error("installer is missing or empty");
  if (!/^[A-Fa-f0-9]{64}$/.test(expectedSha256 ?? "")) throw new Error("expected SHA-256 must contain 64 hexadecimal characters");
  const actualSha256 = createHash("sha256").update(installerBytes).digest("hex");
  if (actualSha256.toLowerCase() !== expectedSha256.toLowerCase()) throw new Error("installer SHA-256 does not match the approved release preflight");
  const normalizedSignature = normalizeSignatureText(signature);
  const signatureLines = normalizedSignature.split(/\r?\n/);
  if (signatureLines.length !== 4 || !signatureLines[0].startsWith("untrusted comment: ") || !/^[A-Za-z0-9+/]+={0,2}$/.test(signatureLines[1]) || !signatureLines[2].startsWith("trusted comment: ") || !/^[A-Za-z0-9+/]+={0,2}$/.test(signatureLines[3])) throw new Error("signature is missing or malformed");
  validateAssetUrl({ mode, version, assetUrl, stagingEndpoint });
  if (typeof notes !== "string" || !notes.trim() || notes.length > 20000) throw new Error("NEWS release notes are missing or exceed 20,000 characters");
  return {
    version,
    notes: notes.trim(),
    platforms: {
      "windows-x86_64": {
        signature: Buffer.from(normalizedSignature, "utf8").toString("base64"),
        url: assetUrl,
      },
    },
  };
}

export function extractReleaseNotes(news, version) {
  const lines = news.split(/\r?\n/);
  const start = lines.findIndex((line) => new RegExp(`^## \\s*v?${version.replaceAll(".", "\\.")}(?:\\s|$)`).test(line));
  if (start < 0) return null;
  let end = lines.findIndex((line, index) => index > start && /^##\s/.test(line));
  if (end < 0) end = lines.length;
  const notes = lines.slice(start + 1, end).join("\n").trim();
  return notes || null;
}

async function main() {
  const required = ["version", "installer", "signature", "sha256", "news", "asset-url", "output"];
  const missing = required.filter((key) => !args[key]);
  if (missing.length) throw new Error(`missing arguments: ${missing.map((key) => `--${key}`).join(", ")}`);
  const repoRoot = resolve(import.meta.dirname, "../..");
  const tauriConfig = JSON.parse(await readFile(resolve(repoRoot, "gui/src-tauri/tauri.conf.json"), "utf8"));
  if (args.version !== tauriConfig.version) throw new Error("version does not match tauri.conf.json");
  const news = await readFile(resolve(repoRoot, args.news), "utf8");
  const notes = extractReleaseNotes(news, args.version);
  if (!notes) throw new Error(`NEWS.md has no release section for ${args.version}`);
  const installerBytes = await readFile(resolve(repoRoot, args.installer));
  const signature = await readFile(resolve(repoRoot, args.signature), "utf8");
  const manifest = createManifest({
    mode: args.mode ?? "stable",
    stagingEndpoint: args["staging-endpoint"],
    version: args.version,
    installerBytes,
    signature,
    expectedSha256: args.sha256,
    notes,
    assetUrl: args["asset-url"],
  });
  await writeFile(resolve(repoRoot, args.output), `${JSON.stringify(manifest, null, 2)}\n`, { flag: "w" });
  console.log(`Generated ${args.output} for ${manifest.version}; installer SHA-256 ${args.sha256.toUpperCase()}`);
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  main().catch(fail);
}
