import forge from "node-forge";

/**
 * RSA-OAEP(SHA-256) encrypt for auth password fields.
 * Matches mei-lang server `Oaep::new::<Sha256>()` decryption.
 */
export function encryptPasswordWithPem(publicKeyPem, text) {
  const pem = String(publicKeyPem || "").trim();
  if (!pem) {
    throw new Error("missing public key pem");
  }
  const plain = String(text ?? "");
  if (!plain) {
    throw new Error("missing password");
  }
  const publicKey = forge.pki.publicKeyFromPem(pem);
  const encrypted = publicKey.encrypt(forge.util.encodeUtf8(plain), "RSA-OAEP", {
    md: forge.md.sha256.create(),
    mgf1: { md: forge.md.sha256.create() },
  });
  return forge.util.encode64(encrypted);
}

if (typeof globalThis !== "undefined") {
  globalThis.MeiAuthRsa = { encryptPasswordWithPem };
}
