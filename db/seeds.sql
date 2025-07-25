-- Database seed data for development and testing

-- Insert sample words for drawing prompts
INSERT OR IGNORE INTO drawing_words (word, difficulty, category) VALUES
  ('cat', 'easy', 'animals'),
  ('dog', 'easy', 'animals'),
  ('house', 'easy', 'objects'),
  ('car', 'easy', 'objects'),
  ('tree', 'easy', 'nature'),
  ('sun', 'easy', 'nature'),
  ('elephant', 'medium', 'animals'),
  ('bicycle', 'medium', 'objects'),
  ('mountain', 'medium', 'nature'),
  ('airplane', 'medium', 'objects'),
  ('butterfly', 'medium', 'animals'),
  ('guitar', 'medium', 'objects'),
  ('octopus', 'hard', 'animals'),
  ('telescope', 'hard', 'objects'),
  ('volcano', 'hard', 'nature'),
  ('microscope', 'hard', 'objects'),
  ('chameleon', 'hard', 'animals'),
  ('lighthouse', 'hard', 'objects');

-- Insert default game settings
INSERT OR IGNORE INTO game_settings (key, value, description) VALUES
  ('default_round_time', '60', 'Default time per round in seconds'),
  ('max_players_per_room', '8', 'Maximum number of players allowed in a room'),
  ('points_correct_guess', '100', 'Base points for a correct guess'),
  ('points_time_bonus', '50', 'Maximum bonus points for fast guessing'),
  ('min_players_to_start', '2', 'Minimum players required to start a game');

-- Create a test room for development (only in development environment)
-- This will be filtered out in production deployments
INSERT OR IGNORE INTO rooms (id, name, host_id, max_players, is_active) VALUES
  ('test-room-dev', 'Development Test Room', 'dev-host', 4, true);

-- Create test players for development
INSERT OR IGNORE INTO players (id, name, room_id, is_host) VALUES
  ('dev-host', 'Dev Host', 'test-room-dev', true),
  ('dev-player-1', 'Test Player 1', 'test-room-dev', false),
  ('dev-player-2', 'Test Player 2', 'test-room-dev', false);