import { defineConfig } from "$fresh/server.ts";
import tailwind from "$fresh/plugins/tailwind.ts";

export default defineConfig({
  plugins: [tailwind()],
  server: {
    // Server configuration
    port: 3000,
  },
  build: {
    target: ["chrome99", "firefox99", "safari15"],
  },
});
