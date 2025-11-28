use serde::{Serialize, Deserialize};
use uuid::Uuid;
use anyhow::Result;
use chrono::{DateTime, Utc};
use num_traits::cast::ToPrimitive;

use crate::Db;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSymbol {
    X,
    O
}

#[derive(Debug)]
pub struct Game {
    pub id: Uuid,
    pub room_id: Uuid,
    pub player_x_id: Option<Uuid>,
    pub player_o_id: Option<Uuid>,
    pub winner_id: Option<Uuid>,
    pub board_state: Vec<Option<PlayerSymbol>>,
    pub moves_count: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateGameRequest {
    pub room_id: Uuid,
    pub player_x_id: Option<Uuid>,
    pub player_o_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateGameResponse {
    pub id: Uuid,
}

impl Db {
    pub async fn create_game(&self, req: CreateGameRequest) -> Result<CreateGameResponse> {
        let game = sqlx::query_as!(
            CreateGameResponse,
            "INSERT INTO games (room_id, player_x_id, player_o_id) VALUES ($1, $2, $3) RETURNING id",
            req.room_id,
            req.player_x_id,
            req.player_o_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(game)
    }

    pub async fn finish_game(
        &self,
        game_id: Uuid,
        winner_id: Option<Uuid>,
        board_state: &[Option<PlayerSymbol>],
        moves_count: i32,
    ) -> Result<()> {
        let board_json = serde_json::to_value(board_state)?;

        sqlx::query!(
            "UPDATE games SET winner_id = $1, board_state = $2, moves_count = $3, finished_at = NOW(), status = 'finished' WHERE id = $4",
            winner_id,
            board_json,
            moves_count,
            game_id
        )
        .execute(&self.pool)
        .await?;

        if let Some(winner) = winner_id {
            sqlx::query!("UPDATE users SET games_played = games_played + 1, games_won = games_won + 1 WHERE id = $1", winner)
                .execute(&self.pool)
                .await?;

            sqlx::query!(
                "UPDATE users SET win_rate = ROUND((games_won::decimal / games_played) * 100, 2) WHERE id = $1",
                winner
            )
            .execute(&self.pool)
            .await?;
        }

        if let Some(game) = sqlx::query!("SELECT player_x_id, player_o_id FROM games WHERE id = $1", game_id)
            .fetch_optional(&self.pool)
            .await?
        {
            if let Some(player_x) = game.player_x_id {
                sqlx::query!("UPDATE users SET games_played = games_played + 1 WHERE id = $1 AND id != $2", player_x, winner_id.unwrap_or(Uuid::nil()))
                    .execute(&self.pool)
                    .await?;

                if winner_id.is_none() || winner_id != Some(player_x) {
                    sqlx::query!(
                        "UPDATE users SET win_rate = ROUND((games_won::decimal / games_played) * 100, 2) WHERE id = $1",
                        player_x
                    )
                    .execute(&self.pool)
                    .await?;
                }
            }

            if let Some(player_o) = game.player_o_id {
                sqlx::query!("UPDATE users SET games_played = games_played + 1 WHERE id = $1 AND id != $2", player_o, winner_id.unwrap_or(Uuid::nil()))
                    .execute(&self.pool)
                    .await?;

                if winner_id.is_none() || winner_id != Some(player_o) {
                    sqlx::query!(
                        "UPDATE users SET win_rate = ROUND((games_won::decimal / games_played) * 100, 2) WHERE id = $1",
                        player_o
                    )
                    .execute(&self.pool)
                    .await?;
                }
            }
        }

        Ok(())
    }

    pub async fn get_user_stats(&self, user_id: Uuid) -> Result<(i32, i32, f32)> {
        let stats = sqlx::query!(
            "SELECT games_played, games_won, win_rate FROM users WHERE id = $1",
            user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok((
            stats.games_played.unwrap_or(0),
            stats.games_won.unwrap_or(0),
            stats.win_rate.unwrap_or(sqlx::types::BigDecimal::from(0)).to_f32().unwrap_or(0.0)
        ))
    }

    pub async fn get_all_user_stats(&self) -> Result<Vec<(Uuid, i32, i32, f32)>> {
        let rows = sqlx::query!(
            "SELECT id, games_played, games_won, win_rate FROM users ORDER BY win_rate DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push((
                row.id,
                row.games_played.unwrap_or(0),
                row.games_won.unwrap_or(0),
                row.win_rate.unwrap_or(sqlx::types::BigDecimal::from(0)).to_f32().unwrap_or(0.0)
            ));
        }

        Ok(stats)
    }
}