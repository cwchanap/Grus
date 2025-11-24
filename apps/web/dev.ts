#!/usr/bin/env -S deno run -A --watch=static/,routes/

import dev from "$fresh/dev.ts";
import config from "./fresh.config.ts";

// Load environment variables safely
try {
  const { load } = await import("$std/dotenv/mod.ts");
  await load({
    allowEmptyValues: true,
    defaultsPath: null, // Don't require .env.example
    examplePath: null, // Disable example validation to prevent warnings
    envPath: ".env",
    export: true, // Export variables to Deno.env
  });
} catch (error) {
  const msg = error instanceof Error ? error.message : String(error);
  console.warn("Warning: Could not load .env file:", msg);
}

await dev(import.meta.url, "./main.ts", config);
