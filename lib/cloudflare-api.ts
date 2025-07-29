// Cloudflare API client for accessing D1 and KV via REST APIs

export interface CloudflareConfig {
  accountId: string;
  apiToken: string;
  databaseId: string;
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


}

// Singleton instance
let cloudflareAPI: CloudflareAPI | null = null;

export function getCloudflareAPI(): CloudflareAPI {
  if (!cloudflareAPI) {
    const config: CloudflareConfig = {
      accountId: Deno.env.get("CLOUDFLARE_ACCOUNT_ID") || "",
      apiToken: Deno.env.get("CLOUDFLARE_API_TOKEN") || "",
      databaseId: Deno.env.get("DATABASE_ID") || "d616e1fe-17e6-4320-aba2-393a60167603",
    };

    if (!config.accountId || !config.apiToken) {
      throw new Error("Missing required Cloudflare API credentials");
    }

    cloudflareAPI = new CloudflareAPI(config);
  }

  return cloudflareAPI;
}
