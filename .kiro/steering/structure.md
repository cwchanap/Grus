# Project Structure

## Directory Organization

```
├── components/          # Reusable UI components (server-side)
├── islands/            # Fresh Islands (client-side interactive components)
├── routes/             # Fresh file-based routing
├── lib/                # Core business logic and utilities
│   ├── dao/           # Data Access Objects for database operations
│   └── websocket/     # WebSocket connection and message handling
├── types/              # TypeScript type definitions
├── db/                 # Database schema, migrations, and seeds
├── static/             # Static assets (CSS, images, favicon)
├── scripts/            # Deployment and setup automation scripts
└── docs/               # Documentation and runbooks
```

## Key Architecture Patterns

### Fresh Framework Structure
- **Routes**: File-based routing in `routes/` directory
- **Islands**: Client-side components in `islands/` for interactivity
- **Components**: Server-side components in `components/` for reusable UI
- **Static**: Assets served directly from `static/` directory

### Business Logic Organization
- **lib/**: Core application logic separated from UI
- **lib/dao/**: Database access layer with separate DAOs for each entity
- **lib/websocket/**: WebSocket handling and real-time communication
- **types/**: Centralized type definitions shared across the application

### Configuration Management
- **lib/config.ts**: Environment-based configuration with validation
- **Environment files**: `.env` for local development, `wrangler.toml` for deployment
- **Database**: Schema in `db/schema.sql`, migrations in `db/migrations/`

## File Naming Conventions
- **Components**: PascalCase (e.g., `DrawingBoard.tsx`, `GameLobby.tsx`)
- **Routes**: kebab-case following Fresh conventions
- **Utilities**: kebab-case (e.g., `game-state-manager.ts`, `drawing-utils.ts`)
- **Types**: kebab-case (e.g., `game.ts`, `websocket.ts`)
- **Tests**: Same name as file being tested with `.test.ts` suffix

## Testing Structure
- **Unit tests**: Co-located with source files in `__tests__/` subdirectories
- **Integration tests**: Separate test files for cross-component functionality
- **Test naming**: `[filename].test.ts` for unit tests, `[feature].integration.test.ts` for integration

## Import Patterns
- **Fresh imports**: Use `$fresh/` prefix for framework imports
- **Standard library**: Use `$std/` prefix for Deno standard library
- **Relative imports**: Use relative paths for local modules
- **Type imports**: Use `import type` for type-only imports

## Database Layer
- **Schema**: Single source of truth in `db/schema.sql`
- **Migrations**: Versioned migration files in `db/migrations/`
- **DAOs**: Separate data access objects for each entity type
- **Seeds**: Test data and initial setup in `db/seeds.sql`