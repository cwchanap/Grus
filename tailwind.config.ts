import { type Config } from "tailwindcss";

export default {
  content: [
    "{routes,islands,components}/**/*.{ts,tsx,js,jsx}",
  ],
  theme: {
    extend: {
      screens: {
        "xs": "475px",
        "touch": { "raw": "(hover: none) and (pointer: coarse)" },
      },
      spacing: {
        "safe-top": "env(safe-area-inset-top)",
        "safe-bottom": "env(safe-area-inset-bottom)",
        "safe-left": "env(safe-area-inset-left)",
        "safe-right": "env(safe-area-inset-right)",
      },
      minHeight: {
        "screen-safe": "calc(100vh - env(safe-area-inset-top) - env(safe-area-inset-bottom))",
      },
      maxHeight: {
        "screen-safe": "calc(100vh - env(safe-area-inset-top) - env(safe-area-inset-bottom))",
      },
      fontSize: {
        "touch": ["16px", "1.5"], // Minimum 16px to prevent zoom on iOS
      },
      animation: {
        "pulse-slow": "pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite",
        "bounce-gentle": "bounce 2s infinite",
      },
    },
  },
  plugins: [
    // Add touch-friendly utilities
    function ({ addUtilities }: any) {
      addUtilities({
        ".touch-manipulation": {
          "touch-action": "manipulation",
        },
        ".touch-none": {
          "touch-action": "none",
        },
        ".touch-pan-x": {
          "touch-action": "pan-x",
        },
        ".touch-pan-y": {
          "touch-action": "pan-y",
        },
        ".no-tap-highlight": {
          "-webkit-tap-highlight-color": "transparent",
        },
        ".no-select": {
          "-webkit-user-select": "none",
          "-moz-user-select": "none",
          "user-select": "none",
        },
        ".no-callout": {
          "-webkit-touch-callout": "none",
        },
        ".safe-area-inset": {
          "padding-top": "env(safe-area-inset-top)",
          "padding-bottom": "env(safe-area-inset-bottom)",
          "padding-left": "env(safe-area-inset-left)",
          "padding-right": "env(safe-area-inset-right)",
        },
      });
    },
  ],
} satisfies Config;
