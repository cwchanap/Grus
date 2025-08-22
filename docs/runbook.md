# Operations Runbook

This runbook provides step-by-step procedures for common operational tasks and incident response for the Multiplayer Drawing Game.

## Quick Reference

### Emergency Contacts
- **Development Team**: [team-email@company.com]
- **DevOps Team**: [devops@company.com]
- **On-Call Engineer**: [oncall@company.com]

### Key URLs
- **Production**: https://your-app.example.com
- **Staging**: https://staging.your-app.example.com
- **Health Check**: `/api/health`
- **Deno Deploy Dashboard**: https://dash.deno.com

### Key Commands
```bash
# Check application health
curl https://your-app.example.com/api/health

# Deploy latest version
deno task deploy  # uses scripts/deploy-deno.sh under the hood

# Rollback deployment
# Use Deno Deploy dashboard to roll back to a previous deployment
```

## Incident Response

### Severity Levels

#### P0 - Critical (Complete Outage)
- **Response Time**: Immediate (< 5 minutes)
- **Examples**: Application completely down, database unavailable
- **Actions**: Page on-call engineer, start incident bridge

#### P1 - High (Major Functionality Impacted)
- **Response Time**: < 15 minutes
- **Examples**: WebSocket connections failing, rooms not loading
- **Actions**: Alert development team, investigate immediately

#### P2 - Medium (Minor Functionality Impacted)
- **Response Time**: < 1 hour
- **Examples**: Slow response times, intermittent errors
- **Actions**: Create ticket, investigate during business hours

#### P3 - Low (Cosmetic Issues)
- **Response Time**: < 24 hours
- **Examples**: UI glitches, minor performance issues
- **Actions**: Create ticket for next sprint

### Incident Response Procedures

#### 1. Initial Response (First 5 minutes)
1. **Acknowledge the alert**
   ```bash
   # Check application status
   curl -s https://your-app.example.com/api/health | jq
   ```

2. **Assess impact**
   - Check Deno Deploy Insights/logs for error rates
   - Review recent deployments
   - Check external dependencies

3. **Communicate**
   - Update incident channel
   - Notify stakeholders if P0/P1

#### 2. Investigation (5-15 minutes)
1. **Check logs**
   - Open Deno Deploy dashboard > your project > Logs
   - Filter for "error" and review recent entries

2. **Check infrastructure**
   - Deno Deploy project status
   - Postgres (if used) connectivity
   - Deno KV availability

3. **Review recent changes**
   - Check recent deployments
   - Review configuration changes
   - Check dependency updates

#### 3. Mitigation (15-30 minutes)
1. **Quick fixes**
   - Roll back to a previous deployment from Deno Deploy dashboard
   - Redeploy current version via `deno task deploy` if needed

2. **Traffic management**
   - Enable maintenance mode if needed
   - Route traffic to backup if available

#### 4. Resolution and Follow-up
1. **Verify fix**
   ```bash
   # Run smoke tests
   deno task smoke-tests:prod
   ```

2. **Document incident**
   - Root cause analysis
   - Timeline of events
   - Lessons learned

## Common Operational Tasks

### Deployment Procedures

#### Standard Deployment
```bash
# 1. Run pre-deployment checks
deno task check
deno task test

# 2. Deploy to staging first
deno task deploy:dev

# 3. Run smoke tests on staging
deno task smoke-tests:staging

# 4. Deploy to production
deno task deploy

# 5. Verify production deployment
deno task smoke-tests:prod
```

#### Emergency Deployment
```bash
# Skip tests for urgent fixes (use with caution)
deno run -A scripts/deploy.ts --skip-tests

# Dry run to validate before deploying
deno task deploy:dry-run
```

#### Rollback Procedures
```bash
# Quick rollback
# Use Deno Deploy dashboard to select and restore a previous deployment

# Verify rollback
curl https://your-app.example.com/api/health
```

### Database Operations

#### Database Health Check (optional Postgres)
```bash
# Postgres example: use your DB console or psql
psql "$DATABASE_URL" -c "SELECT 1;"
```

#### Database Backup (optional Postgres)
- Use your managed Postgres provider's backup/restore features (e.g., Neon)

#### Database Restore (optional Postgres)
- Restore from provider backups and verify via SQL

### Monitoring and Alerting

#### Check Application Metrics
```bash
# Health check with detailed output
curl -s https://your-app.example.com/api/health | jq '.'

# Check specific component
curl -s https://your-app.example.com/api/health | jq '.checks.database'
```

#### View Application Logs
- Use Deno Deploy dashboard > Logs
- Filter by level (ERROR/WARN) or search for "WebSocket"

#### Performance Monitoring
1. **Deno Deploy Insights**
   - Go to Deno Deploy Dashboard > Insights
   - Review request volume, error rates, response times

2. **Custom Metrics**
   - Monitor active game rooms
   - Track WebSocket connections
   - Review database query performance

### Scaling Operations

#### Handle Traffic Spikes
1. **Monitor current load**
```bash
# Check active connections
curl -s https://your-app.example.com/api/health | jq '.performance'
```

2. **Scale resources**
   - Deno Deploy auto-scales
   - Monitor Postgres (if used) limits
   - Check Deno KV quotas

3. **Optimize performance**
   - Enable caching where appropriate
   - Optimize database queries
   - Reduce WebSocket message frequency

#### Capacity Planning
- Monitor daily/weekly usage patterns
- Track resource utilization trends
- Plan for expected traffic increases
- Review and adjust rate limits

## Troubleshooting Guide

### Common Issues

#### 1. Application Won't Start
**Symptoms**: Health check fails, 500 errors
**Diagnosis**:
```bash
# Check logs in Deno Deploy dashboard

# Test locally
deno task start
```
**Solutions**:
- Check environment variables
- Verify database connections
- Review recent configuration changes

#### 2. Database Connection Issues
**Symptoms**: Database health check fails
**Diagnosis**:
```bash
# Test database directly (Postgres example)
psql "$DATABASE_URL" -c "SELECT 1;"
```
**Solutions**:
- Verify DATABASE_URL and provider status (if using Postgres)
- Check database quotas
- Review recent schema changes

#### 3. WebSocket Connection Problems
**Symptoms**: Real-time features not working
**Diagnosis**:
```bash
# Check WebSocket health
curl -s https://your-app.example.com/api/health | jq '.checks.websocket'
```
**Solutions**:
- Verify WebSocket endpoint routing
- Review connection limits
- Review logs in Deno Deploy dashboard

#### 4. High Error Rates
**Symptoms**: Increased 4xx/5xx responses
**Diagnosis**:
- Review logs in Deno Deploy dashboard
- Review recent deployments in Deno Deploy dashboard
**Solutions**:
- Rollback if caused by recent deployment
- Check input validation
- Review rate limiting configuration

#### 5. Slow Response Times
**Symptoms**: High latency, timeouts
**Diagnosis**:
```bash
# Check performance metrics
curl -s https://your-app.example.com/api/health | jq '.performance'
```
**Solutions**:
- Optimize database queries
- Review caching strategy
- Check external API dependencies

### Performance Optimization

#### Database Optimization
```sql
-- Check slow queries
EXPLAIN QUERY PLAN SELECT * FROM rooms WHERE is_active = true;

-- Add indexes if needed
CREATE INDEX idx_rooms_active ON rooms(is_active);
CREATE INDEX idx_players_room ON players(room_id);
```

#### KV Storage Optimization (Deno KV)
- Use appropriate TTL values
- Batch operations when possible
- Monitor storage quotas
- Implement cache invalidation strategy

#### WebSocket Optimization
- Limit message frequency
- Compress large messages
- Implement connection pooling
- Monitor connection counts

## Maintenance Procedures

### Regular Maintenance Tasks

#### Daily
- [ ] Check application health status
- [ ] Review error logs for patterns
- [ ] Monitor resource usage
- [ ] Verify backup completion

#### Weekly
- [ ] Review performance metrics
- [ ] Check security alerts
- [ ] Update dependencies if needed
- [ ] Review and clean up old data

#### Monthly
- [ ] Rotate API tokens
- [ ] Review and update documentation
- [ ] Conduct disaster recovery test
- [ ] Performance optimization review

#### Quarterly
- [ ] Security audit
- [ ] Capacity planning review
- [ ] Update runbook procedures
- [ ] Team training on new procedures

### Scheduled Maintenance

#### Planning
1. **Schedule maintenance window**
   - Notify users in advance
   - Choose low-traffic periods
   - Prepare rollback plan

2. **Prepare maintenance**
   ```bash
   # Create maintenance branch
   git checkout -b maintenance/scheduled-update
   
   # Test changes thoroughly
   deno task test
   deno task deploy:dry-run
   ```

#### Execution
1. **Enable maintenance mode** (if available)
2. **Perform updates**
   ```bash
   # Deploy updates
   deno task deploy
   
   # Verify deployment
   deno task smoke-tests:prod
   ```
3. **Disable maintenance mode**
4. **Monitor for issues**

## Security Procedures

### Security Incident Response
1. **Immediate actions**
   - Assess impact and scope
   - Contain the incident
   - Preserve evidence

2. **Investigation**
   - Review access logs
   - Check for data breaches
   - Identify attack vectors

3. **Recovery**
   - Apply security patches
   - Update credentials
   - Implement additional controls

### Regular Security Tasks
- Monitor for security vulnerabilities
- Review access logs regularly
- Update dependencies for security patches
- Conduct security assessments

## Contact Information

### Escalation Path
1. **Level 1**: Development Team
2. **Level 2**: DevOps Team
3. **Level 3**: Engineering Manager
4. **Level 4**: CTO

### External Contacts
- **Security Team**: [security@company.com]
- **Legal Team**: [legal@company.com] (for data breaches)

## Documentation Updates

This runbook should be updated:
- After each incident
- When procedures change
- During quarterly reviews
- When new team members join

**Last Updated**: [Current Date]
**Next Review**: [Quarterly Review Date]