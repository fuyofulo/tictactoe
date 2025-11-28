use std::{collections::HashMap, sync::Arc};
use actix_web::{HttpRequest, HttpResponse, Responder, post, web, HttpMessage};
use uuid::Uuid;
use tokio::sync::{mpsc};
use serde::{Serialize};

use crate::{state::AppState};

#[derive(Serialize)]
struct CreateRoomResponse {
    room_id: String
}

#[post("/room")]
async fn create_room(req: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
    let _user_id = match req.extensions().get::<Uuid>() {
        Some(&uid) => uid,
        None => return HttpResponse::Unauthorized().finish(),
    };
    
    let room_id = Uuid::new_v4();
    let (tx, rx) = mpsc::channel::<GameCommand>(32);
    
    let state_clone = app_state.clone().into_inner();
    let task_room_id = room_id.clone();
    
    tokio::spawn(async move {
        room_task(task_room_id, rx, state_clone).await;
    });
    
    app_state.active_rooms.insert(room_id, tx);
    
    HttpResponse::Ok().json(CreateRoomResponse {
        room_id: room_id.to_string()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Clone, Serialize)]
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

    pub fn make_move(&mut self, idx: usize) -> Result<bool, String> {
        if idx > 8 {
            return Err("Index out of bounds".to_string());
        }
        if self.board[idx].is_some() {
            return Err("Cell already taken".to_string());
        }
        self.board[idx] = Some(self.current_turn);
        Ok(true)
    }
    
    pub fn switch_turn(&mut self) {
        self.current_turn = match self.current_turn {
            PlayerSymbol::X => PlayerSymbol::O,
            PlayerSymbol::O => PlayerSymbol::X,
        };
    }

    pub fn check_winner(&self) -> Option<PlayerSymbol> {
        let b = self.board;
        let wins = [
            (0, 1, 2), (3, 4, 5), (6, 7, 8), // Rows
            (0, 3, 6), (1, 4, 7), (2, 5, 8), // Cols
            (0, 4, 8), (2, 4, 6)
        ];

        for (x, y, z) in wins.iter() {
            if let (Some(p1), Some(p2), Some(p3)) = (b[*x], b[*y], b[*z]) {
                if p1 == p2 && p2 == p3 {
                    return Some(p1);
                }
            }
        }
        None
    }

    pub fn is_draw(&self) -> bool {
        self.board.iter().all(|&cell| cell.is_some())
    }
}

pub async fn room_task(room_id: Uuid, mut rx: mpsc::Receiver<GameCommand>, state: Arc<AppState>) {
    let mut game = GameState::new(room_id);
    let mut clients: HashMap<Uuid, mpsc::Sender<GameEvent>> = HashMap::new();
    
    println!("Room {} spawned", room_id);
    
    while let Some(cmd) = rx.recv().await {
        match cmd {
            GameCommand::Join { user_id, player_sender } => {
                println!("user {} trying to join", user_id);
                match game.add_player(user_id) {
                    Ok(player_symbol) => {
                        clients.insert(user_id, player_sender.clone());
                        let _ = player_sender.send(GameEvent::GameJoined).await;
                        broadcast_game_state(&mut clients, &game).await;

                        if player_symbol == PlayerSymbol::O {
                            if let Some(first_player_id) = game.player_x {
                                if let Some(tx) = clients.get(&first_player_id) {
                                    let _ = tx.send(GameEvent::OpponentJoined(user_id)).await;
                                }
                            }
                        }
                        println!("player {} joined as {:?}, game status: {:?}", user_id, player_symbol, game.status)
                    }
                    Err(e) => {
                        let _ = player_sender.send(GameEvent::Error(e)).await;
                    }
                }
            }
            GameCommand::Move { user_id, idx } => {
                println!("move attempt: user {}, position {}, game status: {:?}", user_id, idx, game.status);
                if game.status != GameStatus::Active {
                    println!("game not active right now");
                    if let Some(tx) = clients.get(&user_id) {
                        let _ = tx.send(GameEvent::Error("waiting for opponent".to_string())).await;
                    }
                    continue;
                }
                match game.make_move(idx) {
                    Ok(_) => {
                        if let Some(winner_symbol) = game.check_winner() {
                            game.status = GameStatus::Finished;
                            let winner_id = match winner_symbol {
                                PlayerSymbol::X => game.player_x,
                                PlayerSymbol::O => game.player_o
                            };
                            broadcast_game_state(&mut clients, &game).await;
                            let event = GameEvent::GameOver { winner: winner_id };
                            for client in clients.values() {
                                let _ = client.send(event.clone()).await;
                                // todo: sync to db
                            }
                        } else {
                            game.switch_turn();
                            broadcast_game_state(&mut clients, &game).await;
                        }
                    }
                    Err(e) => {
                        if let Some(tx) = clients.get(&user_id) {
                            let _ = tx.send(GameEvent::Error(e)).await;
                        }
                    }
                }
            }
            GameCommand::Leave { user_id } => {
                clients.remove(&user_id);
                if game.status == GameStatus::Active {
                    game.status = GameStatus::Finished;
                    let winner_id = if game.player_x == Some(user_id) {
                        game.player_o
                    } else {
                        game.player_x
                    };
                    if let Some(wid) = winner_id {
                        let event = GameEvent::GameOver { winner: Some(wid) };
                        for client in clients.values() {
                            let _ = client.send(event.clone()).await;
                        }
                        // todo: sync to db
                    }
                }
            }
        }
        if game.status == GameStatus::Finished {
            break;
        }
    }
    state.active_rooms.remove(&room_id);
    println!("room {} closed", room_id);
}

async fn broadcast_game_state(clients: &mut HashMap<Uuid, mpsc::Sender<GameEvent>>, game: &GameState) {
    let event = GameEvent::BoardUpdate(game.board);
    for client in clients.values() {
        let _ = client.send(event.clone()).await;
    }
}

