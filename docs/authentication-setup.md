# Authentication System Setup Guide

## Overview
The optional authentication system has been successfully integrated into the Drawing Game app. Users can play without logging in, but authentication provides personalized features.

## Setup Instructions

### 1. Environment Variables
Create a `.env` file in the project root with the following variables:

```bash
# Neon Database Configuration (required for auth)
DATABASE_URL="postgresql://user:password@host/database?sslmode=require"
DIRECT_URL="postgresql://user:password@host/database?sslmode=require"

# JWT Configuration
JWT_SECRET="your-secret-key-here-minimum-32-chars"
JWT_EXPIRES_IN="7d"

# Session Cookie Configuration
SESSION_COOKIE_NAME="grus_session"
SESSION_COOKIE_SECURE="false"  # Set to "true" in production with HTTPS
SESSION_COOKIE_HTTPONLY="true"
SESSION_COOKIE_SAMESITE="Lax"
```

### 2. Database Setup

#### Option A: Using Neon (Recommended)
1. Sign up at [neon.tech](https://neon.tech)
2. Create a new project
3. Copy the connection strings to your `.env` file

#### Option B: Local PostgreSQL
1. Install PostgreSQL locally
2. Create a database: `createdb grus_db`
3. Update the DATABASE_URL and DIRECT_URL in `.env`

### 3. Initialize Prisma

Run the setup script to generate Prisma client and push schema:

```bash
deno run -A scripts/setup-prisma.ts
```

Or manually:

```bash
# Generate Prisma client
npx prisma generate

# Push schema to database
npx prisma db push
```

### 4. Start the Application

```bash
deno task dev
```

## Features Implemented

### User Authentication
- **Signup**: Create new user accounts at `/signup`
- **Login**: Authenticate existing users at `/login`
- **Logout**: End user sessions
- **Session Management**: JWT-based sessions with secure cookies

### UI Components
- Login/Signup forms with validation
- User status display in main lobby
- Login/Logout buttons
- Error handling and feedback

### Security Features
- Password hashing with bcryptjs
- JWT tokens for sessions
- HttpOnly cookies
- CSRF protection with SameSite cookies
- Automatic session cleanup

## API Endpoints

### Authentication Routes
- `POST /api/auth/signup` - Create new user account
- `POST /api/auth/login` - Authenticate user
- `POST /api/auth/logout` - End session
- `GET /api/auth/me` - Get current user info

## Database Schema

```prisma
model User {
  id        String    @id @default(uuid())
  email     String    @unique
  username  String    @unique
  password  String
  name      String?
  createdAt DateTime  @default(now())
  updatedAt DateTime  @updatedAt
  sessions  Session[]
}

model Session {
  id        String   @id @default(uuid())
  token     String   @unique
  userId    String
  user      User     @relation(fields: [userId], references: [id], onDelete: Cascade)
  expiresAt DateTime
  createdAt DateTime @default(now())
}
```

## Testing the Authentication

1. Click "Login" on the main page
2. Click "Sign up" link to create a new account
3. Fill in the registration form
4. After signup, you'll be logged in automatically
5. Your username will appear in the main lobby
6. Click "Logout" to end your session

## Troubleshooting

### Prisma Client Not Found
If you see "PrismaClient not found" error:
```bash
npx prisma generate
```

### Database Connection Failed
- Verify your DATABASE_URL is correct
- Ensure the database server is running
- Check network connectivity

### Session Not Persisting
- Verify JWT_SECRET is set
- Check cookie settings match your environment
- Ensure SESSION_COOKIE_SECURE matches your HTTPS setup

## Next Steps

- Add password reset functionality
- Implement user profile management
- Add OAuth providers (Google, GitHub)
- Store user game statistics
- Add friend system
