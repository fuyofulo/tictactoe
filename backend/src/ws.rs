use actix_web::{get, web, HttpRequest, HttpResponse, HttpMessage, Error, rt};
use actix_ws::Message;
use futures_util::StreamExt as _; // Needed for stream.next()
use tokio::sync::mpsc;
use uuid::Uuid;
use serde::Deserialize;

use crate::state::AppState;
use crate::routes::room::{GameCommand, GameEvent};

#[derive(Deserialize)]
#[serde(tag = "action", content = "payload")]
enum ClientMessage {
    #[serde(rename = "move")]
    Move(usize),
}

#[get("/ws/{room_id}")]
pub async fn join_room(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<Uuid>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let room_id = path.into_inner();
    
    let user_id = match req.extensions().get::<Uuid>() {
        Some(&uid) => uid,
        None => return Ok(HttpResponse::Unauthorized().finish()), 
    };

    let room_tx = match app_state.active_rooms.get(&room_id) {
        Some(tx) => tx.clone(),
        None => return Ok(HttpResponse::NotFound().body("Room not found")),
    };

    let (user_tx, user_rx) = mpsc::channel::<GameEvent>(32);

    if let Err(_) = room_tx.send(GameCommand::Join { 
        user_id, 
        player_sender: user_tx 
    }).await {
         return Ok(HttpResponse::InternalServerError().body("Room is dead or closed"));
    }

    let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;

    rt::spawn(async move {
        ws_loop(session, msg_stream, user_rx, room_tx, user_id).await;
    });

    Ok(response)
}

async fn ws_loop(
    mut session: actix_ws::Session,
    mut msg_stream: actix_ws::MessageStream,
    mut game_rx: mpsc::Receiver<GameEvent>,
    room_tx: mpsc::Sender<GameCommand>,
    user_id: Uuid,
) {
    loop {
        tokio::select! {
            Some(msg) = msg_stream.next() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(action) = serde_json::from_str::<ClientMessage>(&text) {
                            match action {
                                ClientMessage::Move(idx) => {
                                    let _ = room_tx.send(GameCommand::Move { user_id, idx }).await;
                                }
                            }
                        } else {
                            println!("Invalid JSON from user {}", user_id);
                        }
                    }
                    Ok(Message::Ping(bytes)) => {
                        let _ = session.pong(&bytes).await;
                    }
                    Ok(Message::Close(reason)) => {
                        let _ = session.close(reason).await;
                        break; 
                    }
                    _ => {},
                }
            }

            Some(event) = game_rx.recv() => {
                let json = match serde_json::to_string(&event) {
                    Ok(j) => j,
                    Err(_) => continue,
                };
                
                if let Err(_) = session.text(json).await {
                    break;
                }
            }

            else => break,
        }
    }

    let _ = room_tx.send(GameCommand::Leave { user_id }).await;
    println!("WebSocket closed for user {}", user_id);
}