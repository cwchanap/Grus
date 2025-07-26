# Product Overview

This is a real-time multiplayer drawing and guessing game where players join rooms, take turns drawing prompts, and compete to guess what others are drawing. The game features:

- **Real-time multiplayer gameplay** with WebSocket communication
- **Room-based sessions** supporting up to 8 players per room
- **Drawing canvas** powered by Pixi.js for smooth drawing experience
- **Scoring system** based on guess timing and accuracy
- **Chat functionality** for guessing and communication
- **Persistent game state** with room management and player tracking

The application is designed for web browsers and optimized for both desktop and mobile devices. It's built to scale on Cloudflare's edge infrastructure with global low-latency performance.

## Key User Flows
1. **Join/Create Room**: Players create or join game rooms using room codes
2. **Game Rounds**: Players take turns drawing while others guess
3. **Scoring**: Points awarded based on correct guesses and timing
4. **Real-time Updates**: All players see drawing strokes and chat messages instantly