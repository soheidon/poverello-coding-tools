import test from "node:test";
import assert from "node:assert/strict";
import { decodeTauriPublicKey, validatePublicKeyText, verifyUpdaterSignature } from "./verify-updater-signature.mjs";

const publicKeyText = "untrusted comment: minisign public key: A49EF4746FA6E6D7\nRWTX5qZvdPSepDGWg2bz8n6ArQnBMwgQK1geVosorcbLzcunD+KJ+cYe";
const signatureText = "untrusted comment: signature from tauri secret key\nRUTX5qZvdPSepPF2zsjvP/L/TQtBLRnWRvaq+4omK1UEugtQp/USrGiI1v762sDy3id3aor7bRC9ETyeot6lMHHrWoZ5hjUM1Qc=\ntrusted comment: timestamp:1791649051\tfile:payload\tversion:2.0.0\nDDq2CLOEl5cQmC2eJRHlnZPVViOZsu1CjpPKQoei4A/XBfkDqGO0B+QMrcrHgEQ5qV3mne75tdFWhJpqUXh9AA==";

test("accepts Tauri Minisign bytes and the signed app version", () => {
  assert.equal(verifyUpdaterSignature({ payload: Buffer.from("test"), signatureText, publicKeyText, version: "2.0.0" }).length, 64);
});

test("validates the public-key encoding used by stable builds", () => {
  assert.equal(validatePublicKeyText(publicKeyText).length, 42);
  assert.equal(decodeTauriPublicKey(Buffer.from(publicKeyText).toString("base64")), publicKeyText);
  assert.throws(() => decodeTauriPublicKey("not-a-minisign-key"), /base64-encoded Minisign/);
});

test("rejects changed installers, changed trusted comments, and mismatched versions", () => {
  assert.throws(() => verifyUpdaterSignature({ payload: Buffer.from("tampered"), signatureText, publicKeyText, version: "2.0.0" }), /payload/);
  assert.throws(() => verifyUpdaterSignature({ payload: Buffer.from("test"), signatureText: signatureText.replace("version:2.0.0", "version:2.0.1"), publicKeyText, version: "2.0.1" }), /trusted comment/);
  assert.throws(() => verifyUpdaterSignature({ payload: Buffer.from("test"), signatureText, publicKeyText, version: "2.0.1" }), /does not match/);
  const publicLines = publicKeyText.split("\n");
  const wrongKeyBytes = Buffer.from(publicLines[1], "base64");
  wrongKeyBytes[2] ^= 0x01;
  const wrongPublicKeyText = `${publicLines[0]}\n${wrongKeyBytes.toString("base64")}`;
  assert.throws(() => verifyUpdaterSignature({ payload: Buffer.from("test"), signatureText, publicKeyText: wrongPublicKeyText, version: "2.0.0" }), /key ID does not match/);
});
