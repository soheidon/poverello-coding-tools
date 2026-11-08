import { createHash, createPublicKey, verify as verifySignatureBytes } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const SPKI_ED25519_PREFIX = Buffer.from("302a300506032b6570032100", "hex");

export function validatePublicKeyText(publicKeyText) {
  const publicLines = publicKeyText.trim().split(/\r?\n/);
  if (publicLines.length !== 2 || !publicLines[0].startsWith("untrusted comment: ")) throw new Error("invalid Minisign public key text");
  const publicKeyBytes = Buffer.from(publicLines[1], "base64");
  if (publicKeyBytes.length !== 42 || publicKeyBytes.toString("base64") !== publicLines[1]) throw new Error("invalid Minisign public key encoding");
  if (publicKeyBytes[0] !== 0x45 || publicKeyBytes[1] !== 0x64) throw new Error("unsupported Minisign public key algorithm");
  return publicKeyBytes;
}

export function decodeTauriPublicKey(encodedPublicKey) {
  const decoded = Buffer.from(encodedPublicKey, "base64").toString("utf8");
  if (!encodedPublicKey || Buffer.from(decoded, "utf8").toString("base64").replace(/=+$/, "") !== encodedPublicKey.replace(/=+$/, "")) throw new Error("PCT_UPDATER_PUBLIC_KEY must be base64-encoded Minisign public key text");
  validatePublicKeyText(decoded);
  return decoded;
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

export function verifyUpdaterSignature({ payload, signatureText, publicKeyText, version }) {
  const normalized = normalizeSignatureText(signatureText);
  const signatureLines = normalized.split(/\r?\n/);
  if (signatureLines.length !== 4 || !signatureLines[0].startsWith("untrusted comment: ") || !signatureLines[2].startsWith("trusted comment: ")) throw new Error("invalid Minisign signature text");

  const publicKeyBytes = validatePublicKeyText(publicKeyText);
  const signatureBytes = Buffer.from(signatureLines[1], "base64");
  const globalSignature = Buffer.from(signatureLines[3], "base64");
  if (publicKeyBytes.length !== 42 || signatureBytes.length !== 74 || globalSignature.length !== 64) throw new Error("invalid Minisign key or signature encoding");
  if (!publicKeyBytes.subarray(2, 10).equals(signatureBytes.subarray(2, 10))) throw new Error("signature key ID does not match the public key");
  if (signatureBytes[0] !== 0x45 || signatureBytes[1] !== 0x44) throw new Error("only Tauri's prehashed Minisign signatures are accepted");

  const key = createPublicKey({ key: Buffer.concat([SPKI_ED25519_PREFIX, publicKeyBytes.subarray(10, 42)]), format: "der", type: "spki" });
  const artifactSignature = signatureBytes.subarray(10, 74);
  const digest = createHash("blake2b512").update(payload).digest();
  if (!verifySignatureBytes(null, digest, key, artifactSignature)) throw new Error("installer signature does not match the payload");

  const trustedComment = signatureLines[2].slice("trusted comment: ".length);
  const globalMessage = Buffer.concat([artifactSignature, Buffer.from(trustedComment, "utf8")]);
  if (!verifySignatureBytes(null, globalMessage, key, globalSignature)) throw new Error("Minisign trusted comment signature is invalid");
  const signedVersion = trustedComment.split("\t").find((field) => field.startsWith("version:"))?.slice("version:".length);
  if (!signedVersion) throw new Error("signature has no Tauri app version binding");
  if (signedVersion.replace(/^v/, "") !== version.replace(/^v/, "")) throw new Error(`signature version ${signedVersion} does not match manifest version ${version}`);
  return createHash("sha256").update(payload).digest("hex").toUpperCase();
}

async function main() {
  if (process.argv[2] === "--validate-public-key-env") {
    decodeTauriPublicKey(process.env.PCT_UPDATER_PUBLIC_KEY ?? "");
    console.log("PCT_UPDATER_PUBLIC_KEY format validated.");
    return;
  }
  const [installerPath, signaturePath, publicKeyPath, version] = process.argv.slice(2);
  if (!installerPath || !signaturePath || !publicKeyPath || !version) throw new Error("usage: node verify-updater-signature.mjs <installer> <signature> <public-key> <version>");
  const [payload, signatureText] = await Promise.all([
    readFile(installerPath),
    readFile(signaturePath, "utf8"),
  ]);
  const publicKeyText = publicKeyPath === "-"
    ? Buffer.from(process.env.PCT_UPDATER_PUBLIC_KEY ?? "", "base64").toString("utf8")
    : await readFile(publicKeyPath, "utf8");
  const digest = verifyUpdaterSignature({ payload, signatureText, publicKeyText, version });
  console.log(`Tauri-compatible updater signature verified for v${version}; installer SHA-256 ${digest}`);
}

if (process.argv[1] && resolve(process.argv[1]).toLowerCase() === resolve(import.meta.filename).toLowerCase()) {
  main().catch((error) => {
    console.error(`Updater signature verification failed: ${error.message}`);
    process.exitCode = 1;
  });
}
