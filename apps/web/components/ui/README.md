# shadcn/ui Components for Deno Fresh + Preact

This directory contains shadcn/ui components adapted for use with Deno Fresh and Preact. The components follow the same API and design patterns as the original shadcn/ui library but are optimized for the Deno ecosystem.

## Setup

The setup includes:

1. **Tailwind CSS configuration** with shadcn/ui design tokens
2. **CSS variables** for theming in `static/styles.css`
3. **Utility functions** in `lib/utils.ts` for class merging
4. **Component library** with consistent APIs

## Dependencies

The following dependencies have been added to `deno.json`:

- `class-variance-authority` - For component variants
- `clsx` - For conditional class names
- `tailwind-merge` - For merging Tailwind classes
- `tailwindcss-animate` - For animations
- `lucide-preact` - For icons

## Available Components

### Core Components

- **Button** - Various button styles and sizes
- **Card** - Container component with header, content, and footer
- **Dialog** - Modal dialogs and overlays
- **Badge** - Status indicators and labels
- **Input** - Form input fields
- **Label** - Form labels
- **Select** - Dropdown select components
- **Separator** - Visual dividers

### Usage Examples

```tsx
import { Button, Card, CardContent, CardHeader, CardTitle } from "./ui/index.ts";

// Basic button
<Button>Click me</Button>

// Button variants
<Button variant="outline">Outline</Button>
<Button variant="destructive">Delete</Button>

// Card component
<Card>
  <CardHeader>
    <CardTitle>Card Title</CardTitle>
  </CardHeader>
  <CardContent>
    Card content goes here
  </CardContent>
</Card>
```

## Theming

The components use CSS variables for theming. You can customize the theme by modifying the CSS variables in `static/styles.css`:

```css
:root {
  --primary: 221.2 83.2% 53.3%;
  --primary-foreground: 210 40% 98%;
  /* ... other variables */
}
```

## Mobile Optimization

All components are optimized for mobile devices with:

- Touch-friendly sizing (minimum 44px touch targets)
- Responsive design patterns
- Mobile-specific animations and interactions
- Safe area support for iOS devices

## Accessibility

Components include:

- Proper ARIA attributes
- Keyboard navigation support
- Focus management
- Screen reader compatibility
- High contrast mode support

## Adding New Components

To add a new shadcn/ui component:

1. Create the component file in `components/ui/`
2. Follow the existing patterns for props and styling
3. Use `forwardRef` from `preact/compat` for ref forwarding
4. Export from `components/ui/index.ts`
5. Add proper TypeScript types

## Migration from Existing Components

When migrating existing components:

1. Replace custom styling with shadcn variants
2. Use the `cn()` utility for class merging
3. Update props to match shadcn APIs
4. Test mobile responsiveness
5. Ensure accessibility compliance

## Performance

The components are optimized for:

- Tree shaking (only import what you use)
- Minimal bundle size
- Fast rendering with Preact
- Efficient CSS with Tailwind

## Examples

See `components/ShadcnDemo.tsx` for comprehensive usage examples of all components.
