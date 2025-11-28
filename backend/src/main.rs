use db::Db;
use actix_web::{HttpServer, App, web};
use actix_web::middleware::Logger;
use std::sync::Arc;
use dashmap::DashMap;

use crate::routes::user::{signup, signin, me};
use crate::auth::middleware::JwtAuth;
use state::AppState;

pub mod routes;
pub mod auth;
pub mod state;

#[actix_web::main]
async fn main () {
    dotenvy::dotenv().unwrap();
    let db = Db::new().await.unwrap();
    let active_rooms = Arc::new(DashMap::new());
    
    let app_state = web::Data::new(AppState {
        db: db.clone(),
        active_rooms: active_rooms.clone()
    });
    
    let _ = HttpServer::new( move || {
        App::new()
            .wrap(Logger::default()) 
            .app_data(app_state.clone())
            .service(signup)
            .service(signin)
            .service(
                web::scope("/api")
                    .service(me)
                    .wrap(JwtAuth)
            )    
    })
    .bind("0.0.0.0:3000")
    .unwrap()
    .run()
    .await;
}

