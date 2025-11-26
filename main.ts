/// <reference no-default-lib="true" />
/// <reference lib="dom" />
/// <reference lib="dom.iterable" />
/// <reference lib="dom.asynciterable" />
/// <reference lib="deno.ns" />

// Load environment variables safely from the repo root
try {
  const { load } = await import("$std/dotenv/mod.ts");
  await load({
    allowEmptyValues: true,
    defaultsPath: null,
    examplePath: null,
    envPath: ".env",
  });
} catch (error) {
  const msg = error instanceof Error ? error.message : String(error);
  console.warn("Warning: Could not load .env file:", msg);
}

import { start } from "$fresh/server.ts";
import manifest from "./apps/web/fresh.gen.ts";
import config from "./apps/web/fresh.config.ts";

if (import.meta.main) {
  await start(manifest, config);
}
