# Deployment Guide

This document provides comprehensive instructions for deploying the Multiplayer Drawing Game to Deno Deploy.

## Prerequisites

### Required Tools
- [Deno](https://deno.land/) v1.40.x or later
- [deployctl](https://deno.com/deploy/docs/deployctl) (Deno Deploy CLI):
  ```bash
  deno install -A --no-check -r -f https://deno.land/x/deploy/deployctl.ts
  ```

### Deno Deploy Setup
1. Create a [Deno Deploy account](https://dash.deno.com)
2. Optionally install `deployctl` for CLI deployments (see above)
3. Create a new project in the Deno Deploy dashboard

## Environment Configuration

### Environment Variables
Set the following in your local `.env` and on Deno Deploy:
```bash
# Authentication
JWT_SECRET=your-secret-key-here-minimum-32-chars
JWT_EXPIRES_IN=7d

# Optional: Relational database for auth/user data
DATABASE_URL=postgresql://[user]:[password]@[neon_hostname]/[dbname]?sslmode=require
# For local development (optional): DATABASE_URL=file:./dev.db
```
Notes:
- Game and chat state is stored in Deno KV (no external setup required)
- If you use Postgres via Prisma, provision it separately (e.g., Neon) and set `DATABASE_URL`
- Optional CI secrets: `SLACK_WEBHOOK_URL`, `CODECOV_TOKEN`

## Deployment Methods

### 1. Automated Deployment (Recommended)

The project includes GitHub Actions workflows for CI (format, lint, type-check, tests, build). Deployment is managed via Deno Deploy (manual or GitHub integration).

#### Staging Deployment
- Triggered on pushes to `develop` branch (CI only by default)
- You can connect Deno Deploy GitHub Integration to auto-deploy
- Optionally run smoke tests against your staging URL

#### Production Deployment
- Triggered on pushes to `main` branch (CI only by default)
- Use Deno Deploy dashboard or GitHub Integration to deploy
- Optionally run smoke tests and notifications after deploy

### 2. Manual Deployment

#### Quick Deployment
```bash
# Deploy to development
bash scripts/deploy-deno.sh

# Deploy to production
bash scripts/deploy-deno.sh
```

#### Step-by-Step Deployment

1. **Setup Database**
   If using Postgres, ensure your database is provisioned and `DATABASE_URL` is configured.

2. **Run Tests**
   ```bash
   deno task test
   ```

3. **Build Application**
   ```bash
   deno task build
   ```

4. **Deploy to Deno Deploy**
   ```bash
   # Using deployctl directly
   deployctl deploy --project=grus-multiplayer-drawing-game ./main.ts
   ```

5. **Run Smoke Tests**
   ```bash
   deno run -A scripts/smoke-tests.ts --env=staging
   deno run -A scripts/smoke-tests.ts --env=production
   ```

### 3. Notes
- The repository includes `scripts/deploy-deno.sh` which builds and deploys using `deployctl`
- Configure environment variables in the Deno Deploy dashboard

## Database Notes (optional Postgres/Prisma)
If you enable Postgres for auth/user data:
- Manage schema and migrations using Prisma CLI
- Provision your database (e.g., Neon) and set `DATABASE_URL`

## Monitoring and Alerting

### Setup Monitoring
```bash
# Setup monitoring configuration
deno run -A scripts/monitoring-setup.ts --prod
```
This creates configuration files for:
- Log aggregation settings (if applicable)
- Health check endpoints

### Health Checks

The application includes a health check endpoint at `/api/health` that monitors:
- Database connectivity
- KV storage functionality
- WebSocket support
- Response times
- Memory usage

### Alerting

Configure alerts in your monitoring tool for:
- High error rates (>5% in production, >10% in development)
- Slow response times (>1s in production, >2s in development)
- High memory usage (>80%)

## Troubleshooting

### Common Issues

#### 1. Database Connection Errors
If using Postgres, verify `DATABASE_URL` and connectivity using your database tooling.

#### 2. KV Storage Issues
Use the built-in health endpoint and the provided scripts to inspect state:
```bash
deno task db:inspect
deno task db:inspect:rooms
```

#### 3. WebSocket Connection Problems
- Verify WebSocket endpoint `/api/websocket` is reachable
- Check server logs in Deno Deploy dashboard
- Ensure clients use WSS in production

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
Use the Deno Deploy dashboard to roll back to a previous deployment.

#### 2. Database Rollback
If using Postgres, restore from your managed database backups.

#### 3. Emergency Procedures
1. Temporarily disable the production deployment in Deno Deploy if needed
2. Restore previous deployment
3. Verify health checks pass
4. Re-enable production deployment

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
- Review Deno Deploy Insights or your monitoring dashboard

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
- Check Deno Deploy documentation
- Review application logs in Deno Deploy dashboard
- Use health check endpoints for diagnostics
- Contact development team for application-specific issues

### Useful Commands
```bash
# Run locally
deno task start

# Build
deno task build

# Test locally
deno task start

# Run full test suite
deno task test