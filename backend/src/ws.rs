use actix_web::{get, web, HttpRequest, HttpResponse, HttpMessage, Error, rt};
use actix_ws::Message;
use futures_util::StreamExt as _; // Needed for stream.next()
use tokio::sync::mpsc;
use uuid::Uuid;
use serde::Deserialize;

use crate::state::AppState;
use crate::routes::room::{GameCommand, GameEvent};

// A helper struct to parse incoming JSON from the client
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload")] // e.g., { "action": "move", "payload": 4 }
enum ClientMessage {
    #[serde(rename = "move")]
    Move(usize),
    // Add "chat" or "restart" here later
}

#[get("/ws/{room_id}")]
pub async fn join_room(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<Uuid>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let room_id = path.into_inner();
    
    // 1. Auth: Get User ID 
    let user_id = match req.extensions().get::<Uuid>() {
        Some(&uid) => uid,
        None => return Ok(HttpResponse::Unauthorized().finish()), 
    };

    // 2. Find the Room's Transmitter
    let room_tx = match app_state.active_rooms.get(&room_id) {
        Some(tx) => tx.clone(),
        None => return Ok(HttpResponse::NotFound().body("Room not found")),
    };

    // 3. Create the Private Channel for this user
    // The Room will send events into 'user_tx', we listen on 'user_rx'
    let (user_tx, user_rx) = mpsc::channel::<GameEvent>(32);

    // 4. Send Join Command to Room Task
    // We do this BEFORE the upgrade to ensure the room is actually alive/accepting
    if let Err(_) = room_tx.send(GameCommand::Join { 
        user_id, 
        player_sender: user_tx 
    }).await {
         return Ok(HttpResponse::InternalServerError().body("Room is dead or closed"));
    }

    // 5. Upgrade connection to WebSocket
    // actix_ws::handle returns the response, the session (sender), and the stream (receiver)
    let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;

    // 6. Spawn the Handler Loop
    // We move everything needed into this separate async task
    rt::spawn(async move {
        ws_loop(session, msg_stream, user_rx, room_tx, user_id).await;
    });

    Ok(response)
}

// --- The Main Loop ---
// This handles traffic in BOTH directions
async fn ws_loop(
    mut session: actix_ws::Session,
    mut msg_stream: actix_ws::MessageStream,
    mut game_rx: mpsc::Receiver<GameEvent>,
    room_tx: mpsc::Sender<GameCommand>,
    user_id: Uuid,
) {
    loop {
        // tokio::select! waits for whichever future completes first
        tokio::select! {
            // A. INCOMING FROM CLIENT (User -> Server)
            Some(msg) = msg_stream.next() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        // Parse JSON
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
                        break; // Exit loop
                    }
                    _ => {},
                }
            }

            // B. INCOMING FROM ROOM (Server -> User)
            Some(event) = game_rx.recv() => {
                // Serialize the Rust Enum to JSON
                let json = match serde_json::to_string(&event) {
                    Ok(j) => j,
                    Err(_) => continue,
                };
                
                // Send text frame to WebSocket
                if let Err(_) = session.text(json).await {
                    break; // Client disconnected
                }
            }

            // C. FAIL-SAFE
            else => break, // Both channels closed
        }
    }

    // Cleanup: Notify room that user left
    let _ = room_tx.send(GameCommand::Leave { user_id }).await;
    println!("WebSocket closed for user {}", user_id);
}