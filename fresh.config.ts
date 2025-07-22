import { defineConfig } from "$fresh/server.ts";
import tailwind from "$fresh/plugins/tailwind.ts";

export default defineConfig({
  plugins: [tailwind()],
  server: {
    // Configure for Cloudflare Workers compatibility
    port: 8000,
  },
  build: {
    target: ["chrome99", "firefox99", "safari15"],
  },
});
