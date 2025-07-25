# Deployment Guide

This document provides comprehensive instructions for deploying the Multiplayer Drawing Game to Cloudflare Workers.

## Prerequisites

### Required Tools
- [Deno](https://deno.land/) v1.40.x or later
- [Node.js](https://nodejs.org/) v18 or later (for Wrangler CLI)
- [Wrangler CLI](https://developers.cloudflare.com/workers/wrangler/) v3.x

### Cloudflare Account Setup
1. Create a [Cloudflare account](https://dash.cloudflare.com/sign-up)
2. Obtain your API token from the Cloudflare dashboard
3. Install and authenticate Wrangler CLI:
   ```bash
   npm install -g wrangler
   wrangler login
   ```

## Environment Configuration

### 1. Database Setup

Create Cloudflare D1 databases for each environment:

```bash
# Development database
wrangler d1 create drawing-game-db-dev

# Production database
wrangler d1 create drawing-game-db-prod
```

Update `wrangler.toml` with the database IDs returned from the commands above.

### 2. KV Storage Setup

Create KV namespaces:

```bash
# Development KV
wrangler kv:namespace create "GAME_STATE" --preview

# Production KV
wrangler kv:namespace create "GAME_STATE"
```

Update `wrangler.toml` with the KV namespace IDs.

### 3. Environment Variables

Set up the following environment variables in your CI/CD system:

#### Required Secrets
- `CLOUDFLARE_API_TOKEN`: Your Cloudflare API token
- `STAGING_URL`: URL of your staging deployment
- `PRODUCTION_URL`: URL of your production deployment

#### Optional Secrets
- `SLACK_WEBHOOK_URL`: For deployment notifications
- `CODECOV_TOKEN`: For code coverage reporting

## Deployment Methods

### 1. Automated Deployment (Recommended)

The project includes GitHub Actions workflows for automated deployment:

#### Staging Deployment
- Triggered on pushes to `develop` branch
- Runs tests, builds, and deploys to development environment
- Runs smoke tests for validation

#### Production Deployment
- Triggered on pushes to `main` branch
- Runs full test suite and builds
- Deploys to production environment
- Runs smoke tests and sends notifications

### 2. Manual Deployment

#### Quick Deployment
```bash
# Deploy to development
deno run -A scripts/deploy.ts --dev

# Deploy to production
deno run -A scripts/deploy.ts
```

#### Step-by-Step Deployment

1. **Setup Database**
   ```bash
   deno run -A scripts/setup-db.ts --dev  # for development
   deno run -A scripts/setup-db.ts --prod # for production
   ```

2. **Run Tests**
   ```bash
   deno task test
   ```

3. **Build Application**
   ```bash
   deno task build
   ```

4. **Deploy to Cloudflare**
   ```bash
   wrangler deploy --env development  # for development
   wrangler deploy --env production   # for production
   ```

5. **Run Smoke Tests**
   ```bash
   deno run -A scripts/smoke-tests.ts --env=staging
   deno run -A scripts/smoke-tests.ts --env=production
   ```

### 3. Deployment Script Options

The deployment script supports various options:

```bash
# Dry run (validate without deploying)
deno run -A scripts/deploy.ts --dry-run

# Skip tests (faster deployment)
deno run -A scripts/deploy.ts --skip-tests

# Skip build (if already built)
deno run -A scripts/deploy.ts --skip-build

# Deploy to development
deno run -A scripts/deploy.ts --dev
```

## Database Management

### Running Migrations

Migrations are automatically run during database setup, but can be run manually:

```bash
# List all migrations
ls db/migrations/

# Run specific migration
wrangler d1 execute drawing-game-db-dev --file=db/migrations/001_initial_schema.sql
```

### Database Seeding

Seed data is automatically applied during setup:

```bash
# Manual seeding
wrangler d1 execute drawing-game-db-dev --file=db/seeds.sql
```

### Database Backup

```bash
# Export database
wrangler d1 export drawing-game-db-prod --output=backup.sql

# Import database
wrangler d1 execute drawing-game-db-dev --file=backup.sql
```

## Monitoring and Alerting

### Setup Monitoring

```bash
# Setup monitoring configuration
deno run -A scripts/monitoring-setup.ts --prod
```

This creates configuration files for:
- Cloudflare Analytics alerts
- Log aggregation settings
- Health check endpoints

### Health Checks

The application includes a health check endpoint at `/api/health` that monitors:
- Database connectivity
- KV storage functionality
- WebSocket support
- Response times
- Memory usage

### Alerting

Configure alerts in Cloudflare Dashboard for:
- High error rates (>5% in production, >10% in development)
- Slow response times (>1s in production, >2s in development)
- High memory usage (>80%)

## Troubleshooting

### Common Issues

#### 1. Database Connection Errors
```bash
# Check database status
wrangler d1 info drawing-game-db-prod

# Test database connection
wrangler d1 execute drawing-game-db-prod --command="SELECT 1"
```

#### 2. KV Storage Issues
```bash
# List KV namespaces
wrangler kv:namespace list

# Test KV operations
wrangler kv:key put --binding=GAME_STATE "test-key" "test-value"
wrangler kv:key get --binding=GAME_STATE "test-key"
```

#### 3. WebSocket Connection Problems
- Ensure WebSocket support is enabled in Cloudflare Workers
- Check that Durable Objects are properly configured
- Verify WebSocket endpoints are correctly routed

#### 4. Build Failures
```bash
# Clear Deno cache
deno cache --reload deno.json

# Check TypeScript errors
deno check **/*.ts **/*.tsx

# Verify dependencies
deno info main.ts
```

### Rollback Procedures

#### 1. Quick Rollback
```bash
# Deploy previous version
wrangler rollback
```

#### 2. Database Rollback
```bash
# Restore from backup
wrangler d1 execute drawing-game-db-prod --file=backup-previous.sql
```

#### 3. Emergency Procedures
1. Disable traffic routing in Cloudflare Dashboard
2. Restore previous deployment
3. Verify health checks pass
4. Re-enable traffic routing

## Performance Optimization

### Build Optimization
- Enable tree shaking in build process
- Minimize bundle size
- Optimize asset loading

### Runtime Optimization
- Use KV storage efficiently
- Implement connection pooling
- Optimize WebSocket message handling

### Monitoring Performance
- Track response times
- Monitor memory usage
- Analyze error rates
- Review Cloudflare Analytics

## Security Considerations

### Deployment Security
- Use environment-specific API tokens
- Rotate secrets regularly
- Implement proper access controls
- Monitor for security vulnerabilities

### Runtime Security
- Validate all inputs
- Implement rate limiting
- Use secure WebSocket connections
- Sanitize user data

## Maintenance

### Regular Tasks
- Update dependencies monthly
- Review and rotate API tokens quarterly
- Monitor performance metrics weekly
- Update documentation as needed

### Backup Strategy
- Daily database backups
- Weekly full environment snapshots
- Monthly disaster recovery testing
- Quarterly backup restoration testing

## Support

### Getting Help
- Check Cloudflare Workers documentation
- Review application logs in Cloudflare Dashboard
- Use health check endpoints for diagnostics
- Contact development team for application-specific issues

### Useful Commands
```bash
# View deployment logs
wrangler tail

# Check worker status
wrangler dev --local

# Test locally
deno task start

# Run full test suite
deno task test
```