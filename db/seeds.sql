-- Seed data for development environment
-- This file contains sample data for testing the multiplayer drawing game

-- Insert sample rooms
INSERT OR IGNORE INTO rooms (id, name, host_id, max_players, is_active, created_at) VALUES
('room_001', 'Test Room 1', 'player_001', 8, true, datetime('now', '-1 hour')),
('room_002', 'Art Studio', 'player_002', 6, true, datetime('now', '-30 minutes')),
('room_003', 'Quick Draw', 'player_003', 4, false, datetime('now', '-2 hours'));

-- Insert sample players
INSERT OR IGNORE INTO players (id, name, room_id, is_host, joined_at) VALUES
('player_001', 'Alice', 'room_001', true, datetime('now', '-1 hour')),
('player_002', 'Bob', 'room_002', true, datetime('now', '-30 minutes')),
('player_003', 'Charlie', 'room_003', true, datetime('now', '-2 hours')),
('player_004', 'Diana', 'room_001', false, datetime('now', '-45 minutes')),
('player_005', 'Eve', 'room_001', false, datetime('now', '-40 minutes')),
('player_006', 'Frank', 'room_002', false, datetime('now', '-25 minutes'));

-- Insert sample game sessions
INSERT OR IGNORE INTO game_sessions (id, room_id, winner_id, total_rounds, started_at, ended_at) VALUES
('session_001', 'room_003', 'player_003', 3, datetime('now', '-2 hours'), datetime('now', '-1 hour 30 minutes')),
('session_002', 'room_001', 'player_001', 5, datetime('now', '-1 hour'), NULL);

-- Insert sample scores
INSERT OR IGNORE INTO scores (id, session_id, player_id, points, correct_guesses) VALUES
('score_001', 'session_001', 'player_003', 150, 2),
('score_002', 'session_002', 'player_001', 200, 3),
('score_003', 'session_002', 'player_004', 120, 2),
('score_004', 'session_002', 'player_005', 80, 1);

-- Verify seed data
SELECT 'Rooms created:' as info, COUNT(*) as count FROM rooms
UNION ALL
SELECT 'Players created:', COUNT(*) FROM players
UNION ALL
SELECT 'Game sessions created:', COUNT(*) FROM game_sessions
UNION ALL
SELECT 'Scores created:', COUNT(*) FROM scores;