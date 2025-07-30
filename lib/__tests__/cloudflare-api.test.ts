import { assertEquals, assertRejects } from "$std/testing/asserts.ts";
import { CloudflareAPI } from "../cloudflare-api.ts";

// Mock fetch for testing
const originalFetch = globalThis.fetch;

function mockFetch(responses: Array<{ url?: string; response: Response }>) {
  let callIndex = 0;
  globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const urlString = input.toString();
    const mockResponse = responses[callIndex] || responses[responses.length - 1];
    callIndex++;

    if (mockResponse.url && !urlString.includes(mockResponse.url)) {
      throw new Error(`Unexpected URL: ${urlString}`);
    }

    return mockResponse.response;
  };
}

function restoreFetch() {
  globalThis.fetch = originalFetch;
}

const testConfig = {
  accountId: "test-account-id",
  apiToken: "test-api-token",
  databaseId: "test-database-id",
};

Deno.test("CloudflareAPI - D1 Query Success", async () => {
  const api = new CloudflareAPI(testConfig);

  const mockResult = {
    result: [
      {
        results: [{ id: "1", name: "test" }],
        success: true,
        meta: { duration: 10 },
      },
    ],
  };

  mockFetch([{
    url: "d1/database",
    response: new Response(JSON.stringify(mockResult), { status: 200 }),
  }]);

  const result = await api.executeD1Query("SELECT * FROM test", []);

  assertEquals(result, mockResult.result);
  restoreFetch();
});

Deno.test("CloudflareAPI - D1 Query Error", async () => {
  const api = new CloudflareAPI(testConfig);

  mockFetch([{
    response: new Response("Database error", { status: 500 }),
  }]);

  await assertRejects(
    () => api.executeD1Query("SELECT * FROM test", []),
    Error,
    "Cloudflare API error: 500",
  );

  restoreFetch();
});

// KV tests removed - now using Deno KV instead of Cloudflare KV
