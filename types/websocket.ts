// WebSocket types for Cloudflare Workers

declare global {
  class WebSocketPair {
    0: WebSocket;
    1: WebSocket;
  }

  interface ResponseInit {
    webSocket?: WebSocket;
  }

  interface WebSocket extends EventTarget {
    readonly readyState: number;
    readonly CONNECTING: 0;
    readonly OPEN: 1;
    readonly CLOSING: 2;
    readonly CLOSED: 3;
    
    accept(): void;
    send(data: string | ArrayBuffer | ArrayBufferView): void;
    close(code?: number, reason?: string): void;
    
    addEventListener(type: "message", listener: (event: MessageEvent) => void): void;
    addEventListener(type: "close", listener: (event: CloseEvent) => void): void;
    addEventListener(type: "error", listener: (event: Event) => void): void;
    addEventListener(type: string, listener: EventListener): void;
    
    removeEventListener(type: "message", listener: (event: MessageEvent) => void): void;
    removeEventListener(type: "close", listener: (event: CloseEvent) => void): void;
    removeEventListener(type: "error", listener: (event: Event) => void): void;
    removeEventListener(type: string, listener: EventListener): void;
  }
}

export {};