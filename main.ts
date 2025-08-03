/// <reference no-default-lib="true" />
/// <reference lib="dom" />
/// <reference lib="dom.iterable" />
/// <reference lib="dom.asynciterable" />
/// <reference lib="deno.ns" />

// Load environment variables safely
try {
  const { load } = await import("$std/dotenv/mod.ts");
  await load({ 
    allowEmptyValues: true,
    defaultsPath: null, // Don't require .env.example
    envPath: ".env"
  });
} catch (error) {
  console.warn("Warning: Could not load .env file:", error.message);
  // Set default values for development
  if (!Deno.env.get("CLOUDFLARE_ACCOUNT_ID")) {
    Deno.env.set("CLOUDFLARE_ACCOUNT_ID", "dev-account-id");
  }
  if (!Deno.env.get("CLOUDFLARE_API_TOKEN")) {
    Deno.env.set("CLOUDFLARE_API_TOKEN", "dev-api-token");
  }
}

import { start } from "$fresh/server.ts";
import manifest from "./fresh.gen.ts";
import config from "./fresh.config.ts";

// Start the Fresh server
if (import.meta.main) {
  await start(manifest, config);
}
