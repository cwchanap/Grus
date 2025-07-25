# Multiplayer Drawing Game

A real-time multiplayer drawing and guessing game built with Deno Fresh and deployed to Cloudflare Workers.

## Features

- Real-time multiplayer drawing with Pixi.js
- WebSocket-based communication
- Room-based gameplay
- Scoring system based on guess timing
- Cloudflare D1 database for persistence
- Cloudflare KV for session storage

## Tech Stack

- **Frontend**: Deno Fresh with TypeScript
- **Drawing Engine**: Pixi.js v8
- **Real-time Communication**: WebSockets
- **Database**: Cloudflare D1 (SQLite)
- **Cache/Session**: Cloudflare KV
- **Deployment**: Cloudflare Workers

## Setup

### Prerequisites

- [Deno](https://deno.land/) installed
- [Node.js](https://nodejs.org/) for Wrangler CLI
- Cloudflare account

### Installation

1. Clone the repository
2. Install Wrangler CLI:
   ```bash
   npm install -g wrangler
   ```

3. Login to Cloudflare:
   ```bash
   wrangler login
   ```

4. Set up environment variables:
   ```bash
   cp .env.example .env
   # Edit .env with your values
   ```

### Database Setup

1. Create and set up the database:
   ```bash
   deno run -A scripts/setup-db.ts
   ```

2. For production database:
   ```bash
   deno run -A scripts/setup-db.ts --prod
   ```

3. Update `wrangler.toml` with your database IDs from:
   ```bash
   wrangler d1 list
   ```

### KV Namespace Setup

1. Create KV namespaces:
   ```bash
   wrangler kv:namespace create "GAME_STATE"
   wrangler kv:namespace create "GAME_STATE" --preview
   ```

2. Update `wrangler.toml` with the namespace IDs

### Development

1. Start the development server:
   ```bash
   deno task start
   ```

2. For Cloudflare Workers development:
   ```bash
   deno task dev:worker
   ```

### Deployment

This project includes a comprehensive CI/CD pipeline with automated testing, security scanning, and deployment.

#### Quick Deployment
```bash
# Setup environment (first time only)
deno task setup:env

# Deploy to development
deno task deploy:dev

# Deploy to production
deno task deploy
```

#### CI/CD Pipeline Features
- **Automated Testing**: Runs on every push and PR
- **Security Scanning**: Daily vulnerability scans
- **Performance Testing**: Weekly load testing
- **Dependency Updates**: Automated weekly updates
- **Staging Deployment**: Auto-deploy from `develop` branch
- **Production Deployment**: Auto-deploy from `main` branch
- **Rollback Support**: Automatic rollback on deployment failures
- **Health Checks**: Post-deployment validation
- **Monitoring**: Comprehensive alerting and dashboards

#### Manual Deployment Options
```bash
# Run pre-deployment checks
deno task pre-deploy

# Deploy with options
deno run -A scripts/deploy.ts --dry-run    # Test deployment
deno run -A scripts/deploy.ts --skip-tests # Skip tests (not recommended)
deno run -A scripts/deploy.ts --dev        # Deploy to development
```

See the [deployment guide](docs/deployment.md) for detailed instructions.

## Project Structure

```
├── components/          # Reusable UI components
├── db/                 # Database schema and migrations
├── islands/            # Fresh Islands (client-side components)
├── lib/                # Utility libraries and services
├── routes/             # Fresh routes
├── scripts/            # Deployment and setup scripts
├── static/             # Static assets
├── types/              # TypeScript type definitions
├── fresh.config.ts     # Fresh configuration
├── main.ts             # Application entry point
├── wrangler.toml       # Cloudflare Workers configuration
└── deno.json           # Deno configuration
```

## Configuration

The application uses environment-based configuration:

- `lib/config.ts` - Main configuration file
- `.env` - Environment variables
- `wrangler.toml` - Cloudflare Workers configuration

## Available Scripts

### Development
- `deno task start` - Start development server
- `deno task build` - Build for production
- `deno task test` - Run tests
- `deno task test:watch` - Run tests in watch mode
- `deno task check` - Run linting and type checking

### Deployment
- `deno task deploy` - Deploy to production
- `deno task deploy:dev` - Deploy to development
- `deno task pre-deploy` - Run pre-deployment checks
- `deno task smoke-tests` - Run smoke tests
- `deno task health-check` - Check application health

### Setup
- `deno task setup:env` - Setup environment
- `deno task setup:db` - Setup database
- `deno task setup:monitoring` - Setup monitoring
- `deno task setup:alerts` - Setup alerting

### Monitoring
- `deno task worker:tail` - View live logs
- `deno task worker:rollback` - Rollback deployment

## Environment Variables

See `.env.example` for all available environment variables.

## License

MIT License