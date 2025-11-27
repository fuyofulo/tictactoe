use db::Db;
use actix_web::{HttpServer, App, web};
use actix_web::middleware::Logger;

use crate::routes::user::{signup, signin, me};
use crate::auth::middleware::JwtAuth;

pub mod routes;
pub mod auth;

#[actix_web::main]
async fn main () {
    dotenvy::dotenv().unwrap();
    let db = Db::new().await.unwrap();
    let _ = HttpServer::new( move || {
        App::new()
            .wrap(Logger::default()) 
            .app_data(actix_web::web::Data::new(db.clone()))
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

