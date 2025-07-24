import { type PageProps } from "$fresh/server.ts";
import { ErrorNotifications } from "../components/ErrorBoundary.tsx";

export default function App({ Component }: PageProps) {
  return (
    <html>
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>Drawing Game</title>
        <link rel="stylesheet" href="/styles.css" />
        <style>{`
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
        `}</style>
      </head>
      <body>
        <Component />
        <ErrorNotifications />
      </body>
    </html>
  );
}
