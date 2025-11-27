import { type Config } from "tailwindcss";
import tailwindAnimate from "tailwindcss-animate";

export default {
  content: [
    "./apps/web/{routes,islands,components}/**/*.{ts,tsx,js,jsx}",
    "./apps/web/lib/**/*.{ts,tsx,js,jsx}",
    "./apps/web/static/**/*.{css,html}",
  ],
  darkMode: ["class"],
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: {
        "2xl": "1400px",
      },
    },
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
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
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
      },
    },
  },
  plugins: [
    tailwindAnimate,
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
