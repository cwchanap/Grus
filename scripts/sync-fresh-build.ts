import { copy } from "https://deno.land/std@0.216.0/fs/copy.ts";
import { exists } from "https://deno.land/std@0.216.0/fs/exists.ts";
import { join } from "https://deno.land/std@0.216.0/path/join.ts";

const repoRoot = Deno.cwd();
const webFreshDir = join(repoRoot, "apps", "web", "_fresh");
const rootFreshDir = join(repoRoot, "_fresh");

if (!await exists(webFreshDir)) {
  throw new Error(
    `Expected Fresh build output at ${webFreshDir}, but it does not exist. ` +
      "Make sure 'deno task build' completed successfully.",
  );
}

try {
  await Deno.remove(rootFreshDir, { recursive: true });
} catch (error) {
  if (!(error instanceof Deno.errors.NotFound)) {
    throw error;
  }
}

await copy(webFreshDir, rootFreshDir, { overwrite: true });

console.log(`Synchronized Fresh build to ${rootFreshDir}`);
