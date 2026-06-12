#!/usr/bin/env node
/**
 * Encrypt with forge (auth/rsa-oaep.js) for Rust decrypt compatibility checks.
 * Usage: node scripts/verify-forge-rsa-compat.mjs <public.pem> <plaintext>
 */
import { readFileSync } from "node:fs";
import { encryptPasswordWithPem } from "../app/assets/auth/rsa-oaep.js";

const [publicPemPath, plain] = process.argv.slice(2);
if (!publicPemPath || plain === undefined) {
  console.error("usage: verify-forge-rsa-compat.mjs <public.pem> <plaintext>");
  process.exit(1);
}
const publicPem = readFileSync(publicPemPath, "utf8");
process.stdout.write(encryptPasswordWithPem(publicPem, plain));
