import { type PageProps } from "$fresh/server.ts";

export default function App({ Component }: PageProps) {
  return (
    <html>
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>Drawing Game</title>
        <link rel="stylesheet" href="/styles.css" />
        <script dangerouslySetInnerHTML={{
          __html: `
            // Polyfill to prevent node:process imports
            if (typeof globalThis !== 'undefined') {
              // Ensure crypto.randomUUID is available
              if (!globalThis.crypto || !globalThis.crypto.randomUUID) {
                if (!globalThis.crypto) globalThis.crypto = {};
                globalThis.crypto.randomUUID = function() {
                  return Date.now().toString(36) + Math.random().toString(36).substr(2, 9);
                };
              }
              
              // Prevent Node.js module resolution
              if (typeof globalThis.process === 'undefined') {
                globalThis.process = { env: {} };
              }
            }
          `
        }} />
        <style>
          {`
          @keyframes slide-in {
            from {
              transform: translateX(100%);
              opacity: 0;
            }
            to {
              transform: translateX(0);
              opacity: 1;
            }
          }
          .animate-slide-in {
            animation: slide-in 0.3s ease-out;
          }
        `}
        </style>
      </head>
      <body>
        <Component />
      </body>
    </html>
  );
}
