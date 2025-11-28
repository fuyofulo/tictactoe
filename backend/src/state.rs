use dashmap::DashMap;
use uuid::Uuid;
use tokio::sync::mpsc;
use db::Db;
use crate::routes::room::GameCommand;
use std::sync::Arc;

pub struct AppState {
    pub db: Db,
    pub active_rooms: Arc<DashMap<Uuid, mpsc::Sender<GameCommand>>>
}

