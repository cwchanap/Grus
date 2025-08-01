/**
 * Configuration for lint-staged equivalent
 * Defines which tools to run on which file types
 */

export interface LintStagedConfig {
  [pattern: string]: string | string[];
}

export const config: LintStagedConfig = {
  // TypeScript and JavaScript files
  "*.{ts,tsx,js,jsx}": [
    "deno fmt",
    "deno lint",
    "deno check --unstable-kv",
  ],
  
  // JSON and Markdown files
  "*.{json,jsonc,md}": [
    "deno fmt",
  ],
  
  // Configuration files
  "*.{toml,yaml,yml}": [
    // Add any specific tools for config files if needed
  ],
};

export const excludePatterns = [
  "**/_fresh/*",
  "**/node_modules/*",
  "**/.git/*",
  "**/coverage/*",
  "fresh.gen.ts",
  "**/*.min.js",
  "**/*.bundle.js",
];