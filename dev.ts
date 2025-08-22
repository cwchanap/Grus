#!/usr/bin/env -S deno run -A --watch=static/,routes/

import dev from "$fresh/dev.ts";
import config from "./fresh.config.ts";

// Load environment variables safely
try {
  const { load } = await import("$std/dotenv/mod.ts");
  await load({
    allowEmptyValues: true,
    defaultsPath: null, // Don't require .env.example
    envPath: ".env",
  });
} catch (error) {
  const msg = error instanceof Error ? error.message : String(error);
  console.warn("Warning: Could not load .env file:", msg);
}

await dev(import.meta.url, "./main.ts", config);
