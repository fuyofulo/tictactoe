-- Create games table
CREATE TABLE IF NOT EXISTS games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id UUID NOT NULL,
    player_x_id UUID REFERENCES users(id),
    player_o_id UUID REFERENCES users(id),
    winner_id UUID REFERENCES users(id), -- NULL for draw
    board_state JSONB, -- Store final board as array of nullable symbols
    moves_count INTEGER DEFAULT 0,
    started_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    finished_at TIMESTAMP WITH TIME ZONE,
    status VARCHAR(20) DEFAULT 'active' -- active, finished, abandoned
);

-- Add game statistics to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS games_played INTEGER DEFAULT 0;
ALTER TABLE users ADD COLUMN IF NOT EXISTS games_won INTEGER DEFAULT 0;
ALTER TABLE users ADD COLUMN IF NOT EXISTS win_rate DECIMAL(5,2) DEFAULT 0.00;

-- Create indexes
CREATE INDEX idx_games_room_id ON games(room_id);
CREATE INDEX idx_games_player_x_id ON games(player_x_id);
CREATE INDEX idx_games_player_o_id ON games(player_o_id);
CREATE INDEX idx_games_winner_id ON games(winner_id);
CREATE INDEX idx_games_status ON games(status);