// Cloudflare API client for accessing D1 and KV via REST APIs

export interface CloudflareConfig {
  accountId: string;
  apiToken: string;
  databaseId: string;
  kvNamespaceId: string;
}

export class CloudflareAPI {
  private config: CloudflareConfig;
  private baseUrl: string;

  constructor(config: CloudflareConfig) {
    this.config = config;
    this.baseUrl = `https://api.cloudflare.com/client/v4/accounts/${config.accountId}`;
  }

  private async makeRequest(endpoint: string, options: RequestInit = {}): Promise<any> {
    const url = `${this.baseUrl}${endpoint}`;
    const response = await fetch(url, {
      ...options,
      headers: {
        "Authorization": `Bearer ${this.config.apiToken}`,
        "Content-Type": "application/json",
        ...options.headers,
      },
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Cloudflare API error: ${response.status} ${error}`);
    }

    return await response.json();
  }

  // D1 Database methods
  async executeD1Query(sql: string, params: any[] = []): Promise<any> {
    const endpoint = `/d1/database/${this.config.databaseId}/query`;

    const body = {
      sql,
      params,
    };

    const result = await this.makeRequest(endpoint, {
      method: "POST",
      body: JSON.stringify(body),
    });

    return result.result;
  }

  async executeD1Batch(statements: Array<{ sql: string; params?: any[] }>): Promise<any> {
    const endpoint = `/d1/database/${this.config.databaseId}/query`;

    const body = statements.map((stmt) => ({
      sql: stmt.sql,
      params: stmt.params || [],
    }));

    const result = await this.makeRequest(endpoint, {
      method: "POST",
      body: JSON.stringify(body),
    });

    return result.result;
  }

  // KV methods
  async kvGet(key: string): Promise<string | null> {
    try {
      const endpoint = `/storage/kv/namespaces/${this.config.kvNamespaceId}/values/${
        encodeURIComponent(key)
      }`;
      const response = await fetch(`${this.baseUrl}${endpoint}`, {
        headers: {
          "Authorization": `Bearer ${this.config.apiToken}`,
        },
      });

      if (response.status === 404) {
        return null;
      }

      if (!response.ok) {
        throw new Error(`KV GET error: ${response.status}`);
      }

      return await response.text();
    } catch (error) {
      console.error("KV GET error:", error);
      return null;
    }
  }

  async kvPut(
    key: string,
    value: string,
    options: { expirationTtl?: number; metadata?: any } = {},
  ): Promise<void> {
    const endpoint = `/storage/kv/namespaces/${this.config.kvNamespaceId}/values/${
      encodeURIComponent(key)
    }`;

    const url = new URL(`${this.baseUrl}${endpoint}`);
    if (options.expirationTtl) {
      url.searchParams.set("expiration_ttl", options.expirationTtl.toString());
    }

    const headers: Record<string, string> = {
      "Authorization": `Bearer ${this.config.apiToken}`,
    };

    if (options.metadata) {
      headers["CF-KV-Metadata"] = JSON.stringify(options.metadata);
    }

    const response = await fetch(url.toString(), {
      method: "PUT",
      headers,
      body: value,
    });

    if (!response.ok) {
      throw new Error(`KV PUT error: ${response.status}`);
    }
  }

  async kvDelete(key: string): Promise<void> {
    const endpoint = `/storage/kv/namespaces/${this.config.kvNamespaceId}/values/${
      encodeURIComponent(key)
    }`;

    const response = await fetch(`${this.baseUrl}${endpoint}`, {
      method: "DELETE",
      headers: {
        "Authorization": `Bearer ${this.config.apiToken}`,
      },
    });

    if (!response.ok && response.status !== 404) {
      throw new Error(`KV DELETE error: ${response.status}`);
    }
  }

  async kvList(options: { prefix?: string; limit?: number; cursor?: string } = {}): Promise<any> {
    const endpoint = `/storage/kv/namespaces/${this.config.kvNamespaceId}/keys`;
    const url = new URL(`${this.baseUrl}${endpoint}`);

    if (options.prefix) url.searchParams.set("prefix", options.prefix);
    if (options.limit) url.searchParams.set("limit", options.limit.toString());
    if (options.cursor) url.searchParams.set("cursor", options.cursor);

    const result = await this.makeRequest(url.pathname + url.search);
    return result.result;
  }
}

// Singleton instance
let cloudflareAPI: CloudflareAPI | null = null;

export function getCloudflareAPI(): CloudflareAPI {
  if (!cloudflareAPI) {
    const config: CloudflareConfig = {
      accountId: Deno.env.get("CLOUDFLARE_ACCOUNT_ID") || "",
      apiToken: Deno.env.get("CLOUDFLARE_API_TOKEN") || "",
      databaseId: Deno.env.get("DATABASE_ID") || "d616e1fe-17e6-4320-aba2-393a60167603",
      kvNamespaceId: Deno.env.get("KV_NAMESPACE_ID") || "bea0c6d861e7477fae40b0e9c126ed30",
    };

    if (!config.accountId || !config.apiToken) {
      throw new Error("Missing required Cloudflare API credentials");
    }

    cloudflareAPI = new CloudflareAPI(config);
  }

  return cloudflareAPI;
}
