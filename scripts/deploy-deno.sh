#!/bin/bash

# Deployment script for Deno Deploy
# This script deploys the Fresh application to Deno Deploy

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

PROJECT_NAME="grus-multiplayer-drawing-game"

echo -e "${BLUE}🚀 Deploying ${PROJECT_NAME} to Deno Deploy${NC}"

# Check if deployctl is installed
if ! command -v deployctl &> /dev/null; then
    echo -e "${YELLOW}⚠️  deployctl not found. Installing...${NC}"
    deno install -A --no-check -r -f https://deno.land/x/deploy/deployctl.ts
fi

# Check if user is logged in
if ! deployctl --help &> /dev/null; then
    echo -e "${RED}❌ deployctl not working. Please install it manually:${NC}"
    echo "deno install -A --no-check -r -f https://deno.land/x/deploy/deployctl.ts"
    exit 1
fi

echo -e "${GREEN}✅ deployctl found${NC}"

# Build the Fresh application
echo -e "${BLUE}📦 Building Fresh application...${NC}"
deno task build

# Deploy to Deno Deploy
echo -e "${BLUE}🚀 Deploying to Deno Deploy...${NC}"

# Check if project exists, if not create it
echo -e "${BLUE}📋 Deploying project: ${PROJECT_NAME}${NC}"

deployctl deploy \
  --project="${PROJECT_NAME}" \
  --entrypoint="./main.ts" \
  --description="Grus Multiplayer Drawing Game - Fresh Application" \
  --env-file=".env" \
  ./

if [ $? -eq 0 ]; then
    echo -e "\n${GREEN}🎉 Deployment successful!${NC}"
    echo -e "${BLUE}📋 Your application is now live at:${NC}"
    echo -e "${GREEN}https://${PROJECT_NAME}.deno.dev${NC}"
    
    echo -e "\n${BLUE}💡 Next steps:${NC}"
    echo -e "1. Set up environment variables in Deno Deploy dashboard (e.g. JWT_SECRET, DATABASE_URL, ENVIRONMENT)"
    echo -e "2. Test the health endpoint: ${GREEN}https://${PROJECT_NAME}.deno.dev/api/health${NC}"
else
    echo -e "\n${RED}❌ Deployment failed${NC}"
    exit 1
fi

echo -e "\n${BLUE}🔧 Example Environment Variables (configure as needed):${NC}"
echo -e "- ENVIRONMENT: production"
echo -e "- JWT_SECRET: (required for auth)"
echo -e "- DATABASE_URL: (e.g. Postgres/Neon or file:./dev.db for local)"

echo -e "\n${GREEN}✨ Deployment completed!${NC}"