import { assertEquals } from "$std/assert/mod.ts";

Deno.test("simple test", () => {
  assertEquals(1 + 1, 2);
});