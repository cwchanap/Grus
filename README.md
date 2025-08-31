# 🎨 Grus - Multiplayer Drawing Game

A real-time multiplayer drawing and guessing game built with Fresh (Deno), featuring WebSocket communication, persistent game state, and responsive design.

## ✨ Features

- **Real-time multiplayer gameplay** with WebSocket communication
- **Room-based sessions** supporting up to 8 players per room
- **Drawing canvas** powered by Pixi.js for smooth drawing experience
- **Scoring system** based on guess timing and accuracy
- **Chat functionality** for guessing and communication
- **Persistent game state** with room management and player tracking
- **Mobile-optimized** interface with touch support

## 🛠️ Tech Stack

- **Runtime**: Deno with TypeScript
- **Framework**: Fresh (Deno's full-stack web framework)
- **Frontend**: Preact with JSX
- **Styling**: Tailwind CSS
- **Drawing Engine**: Pixi.js v8
- **Real-time Communication**: WebSockets
- **State Storage**: Deno KV (built-in key-value store)
- **Optional Relational DB**: Postgres (e.g., Neon) via Prisma for auth/user data
- **Deployment**: Deno Deploy

## 🚀 Getting Started

### Prerequisites

- [Deno](https://deno.land/) (v2.1+)

### Installation

1. Clone the repository:

```bash
git clone <repository-url>
cd grus
```

2. Set up environment variables:

```bash
cp .env.example .env
# Edit .env with your application secrets (e.g., JWT_SECRET, DATABASE_URL)
```

3. Set up the database (if using authentication):

```bash
deno run -A scripts/setup-prisma.ts
```

4. Install dependencies (Deno handles this automatically):

```bash
deno task check
```

## Test Accounts

For testing and development purposes, you can use these test accounts:

### Test Account 1

- **Email**: test@example.com
- **Username**: testuser
- **Password**: password123
- **Name**: Test User

### Test Account 2 (Verified Working)

- **Email**: test2@example.com
- **Username**: testuser2
- **Password**: password123
- **Name**: Test User 2

You can log in with either the email or username for both accounts. Both accounts have been tested and verified to work with the authentication system.

### Development

Start the development server:

```bash
deno task start
```

The application will be available at `http://localhost:3000`.

### Environment Variables

Required environment variables:

```bash
# Authentication
JWT_SECRET=your-secret-key-here-minimum-32-chars

# Optional: Relational database for auth/user data (e.g., Neon Postgres)
DATABASE_URL=postgresql://[user]:[password]@[neon_hostname]/[dbname]?sslmode=require
```

**Note**: JWT expiration time and session cookie configuration are now managed in the application configuration (`lib/config.ts`) rather than environment variables for better maintainability.

## 🚀 Deployment

### Deno Deploy (Recommended)

1. Build the application:

```bash
deno task build
```

2. Deploy to Deno Deploy:

```bash
bash scripts/deploy-deno.sh
```

3. Set environment variables in the Deno Deploy dashboard:
   - `JWT_SECRET`
   - `DATABASE_URL` (optional, if using Postgres/Prisma)

### Manual Deployment

You can also deploy manually using `deployctl`:

```bash
deployctl deploy --project=grus-multiplayer-drawing-game ./main.ts
```

## 🏗️ Infrastructure

The application stores transient and gameplay state in **Deno KV**. Optionally, authentication/user data can be stored in **Postgres** via Prisma (e.g., Neon on serverless).

## 📁 Project Structure

```
├── components/          # Reusable UI components (server-side)
├── islands/            # Fresh Islands (client-side interactive components)
├── routes/             # Fresh file-based routing
├── lib/                # Core business logic and utilities
│   ├── auth/prisma-client.ts   # Prisma client init (optional Postgres)
│   ├── db/                     # Data services (Deno KV)
│   │   ├── kv-service.ts       # KV storage operations
│   │   └── index.ts            # DB module exports
│   └── websocket/           # WebSocket handling
├── types/              # TypeScript type definitions
├── static/             # Static assets
└── scripts/            # Deployment scripts
```

## 🔧 API Endpoints

- `GET /api/health` - Health check endpoint
- `GET /api/rooms` - List active game rooms
- `POST /api/rooms` - Create a new game room
- `WebSocket /api/websocket` - Real-time game communication

## 🎮 Game Flow

1. **Lobby**: Players can create or join game rooms
2. **Room Setup**: Host configures game settings
3. **Gameplay**: Players take turns drawing while others guess
4. **Scoring**: Points awarded based on correct guesses and timing
5. **Results**: Final scores and winner announcement

## 🧪 Testing

Run tests:

```bash
deno task test
```

Run tests with coverage:

```bash
deno task test:coverage
```

## 🔍 Health Check

After deployment, verify the application is working:

```bash
curl https://your-app.deno.dev/api/health
```

Expected response:

```json
{
  "status": "ok",
  "timestamp": "2024-01-01T00:00:00.000Z",
  "environment": "production",
  "checks": {
    "database": { "status": "ok", "latency": 123 },
    "kv_storage": { "status": "ok", "latency": 45 },
    "websocket": { "status": "ok" }
  }
}
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature-name`
3. Make your changes and add tests
4. Run tests: `deno task test`
5. Commit your changes: `git commit -am 'Add feature'`
6. Push to the branch: `git push origin feature-name`
7. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🆘 Support

For support, please open an issue on GitHub or contact the development team.

---

**Live Demo**: [https://grus-multiplayer-drawing-game.deno.dev](https://grus-multiplayer-drawing-game.deno.dev) (after deployment)
