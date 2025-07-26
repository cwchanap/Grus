# Technology Stack

## Core Technologies
- **Runtime**: Deno with TypeScript
- **Framework**: Fresh (Deno's full-stack web framework)
- **Frontend**: Preact with JSX
- **Styling**: Tailwind CSS
- **Drawing Engine**: Pixi.js v8
- **Real-time Communication**: WebSockets
- **Database**: Cloudflare D1 (SQLite)
- **Cache/Session Storage**: Cloudflare KV
- **Deployment**: Cloudflare Workers

## Build System
- **Package Manager**: Deno (no npm/yarn needed)
- **Task Runner**: Deno tasks (defined in `deno.json`)
- **Bundler**: Fresh's built-in bundler
- **Linting**: Deno's built-in linter with Fresh rules
- **Testing**: Deno's built-in test runner

## Common Commands

### Development
```bash
deno task start          # Start development server with hot reload
deno task check          # Run linting, formatting, and type checking
deno task test           # Run all tests
deno task test:watch     # Run tests in watch mode
deno task test:coverage  # Run tests with coverage report
```

### Building & Deployment
```bash
deno task build          # Build for production
deno task preview        # Preview production build locally
deno task deploy         # Deploy to production
deno task deploy:dev     # Deploy to development environment
deno task pre-deploy     # Run pre-deployment checks
```

### Environment Setup
```bash
deno task setup:env      # Setup environment variables
deno task setup:db       # Setup database and run migrations
deno task setup:monitoring # Setup monitoring and alerts
```

### Cloudflare Workers
```bash
deno task dev:worker     # Start Cloudflare Workers development
deno task worker:tail    # View live logs from deployed worker
deno task worker:rollback # Rollback to previous deployment
```

## Key Dependencies
- `$fresh/`: Fresh framework (v1.7.3)
- `preact`: UI library (v10.22.0)
- `@preact/signals`: State management
- `pixi.js`: 2D graphics rendering (v8.0.0)
- `tailwindcss`: CSS framework (v3.4.1)
- `$std/`: Deno standard library

## Configuration Files
- `deno.json`: Deno configuration, tasks, and imports
- `fresh.config.ts`: Fresh framework configuration
- `wrangler.toml`: Cloudflare Workers deployment config
- `tailwind.config.ts`: Tailwind CSS configuration
- `lib/config.ts`: Application configuration and environment variables