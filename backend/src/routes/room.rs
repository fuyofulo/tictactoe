use uuid::Uuid;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSymbol {
    X, 
    O
}

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub enum GameStatus {
    WaitingForPlayers,
    Active,
    Finished
}

pub struct GameState {
    pub room_id: Uuid,
    pub board: [Option<PlayerSymbol>; 9],
    pub current_turn: PlayerSymbol,
    pub status: GameStatus,
    pub player_x: Option<Uuid>,
    pub player_o: Option<Uuid>
}

pub enum GameCommand {
    Join {
        user_id: Uuid,
        player_sender: mpsc::Sender<GameEvent> ,
    },
    Move {
        user_id: Uuid,
        idx: usize,
    },
    Leave {
        user_id: Uuid,
    }
}

pub enum GameEvent {
    GameJoined,
    OpponentJoined(Uuid),
    BoardUpdate([Option<PlayerSymbol>; 9]),
    GameOver { winner: Option<Uuid> },
    Error(String),
}

impl GameState {
    pub fn new(room_id: Uuid) -> Self {
        Self {
            room_id,
            board: [None; 9],
            current_turn: PlayerSymbol::X,
            status: GameStatus::WaitingForPlayers,
            player_x: None,
            player_o: None
        }
    }
    
    pub fn add_player(&mut self, player_id: Uuid) -> Result<PlayerSymbol, String> {
        if self.status != GameStatus::WaitingForPlayers {
            return Err("Game is either finished or full".to_string());
        }
        if self.player_x.is_none() {
            self.player_x = Some(player_id);
            Ok(PlayerSymbol::X)
        } else if self.player_o.is_none() {
            self.player_o = Some(player_id);
            self.status = GameStatus::Active;
            Ok(PlayerSymbol::O)
        } else {
            Err("Room is full".to_string())
        }
    }
    
    pub fn is_turn(&self, player_id: Uuid) -> bool {
        match self.current_turn {
            PlayerSymbol::X => self.player_x == Some(player_id),
            PlayerSymbol::O => self.player_o == Some(player_id)
        }
    }
}
