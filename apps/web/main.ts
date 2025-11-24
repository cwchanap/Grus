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
    examplePath: null, // Disable example validation to prevent warnings
    envPath: ".env",
  });
} catch (error) {
  const msg = error instanceof Error ? error.message : String(error);
  console.warn("Warning: Could not load .env file:", msg);
}

import { start } from "$fresh/server.ts";
import manifest from "./fresh.gen.ts";
import config from "./fresh.config.ts";

// Start the Fresh server
if (import.meta.main) {
  await start(manifest, config);
}
