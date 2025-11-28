use actix_web::{HttpResponse,HttpRequest, HttpMessage, Responder, post, web, get};
use serde::{Serialize, Deserialize};
use argon2::{Argon2, PasswordHasher, PasswordVerifier, password_hash::{PasswordHash, SaltString, rand_core::OsRng}};
use uuid::Uuid;
use crate::{auth::jwt::create_jwt_for_user, state::AppState};
#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[post("/signup")]
async fn signup(app_state: web::Data<AppState>, body: web::Json<LoginRequest>) -> impl Responder {
    
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = match argon2.hash_password(body.password.as_bytes(), &salt) {
        Ok(phc) => phc.to_string(),
        Err(_) => return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "failed to hash password"
        }))
    };
    println!("{}", password_hash);
    
    let result = app_state.db.create_user(&body.username, &password_hash).await;
    
    match result {
        Ok(user) => {
            HttpResponse::Ok().json(serde_json::json!({
                "message": "user created successfully",
                "id": user.id
            }))
        }
        Err(_e) => {
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "failed to create user"
            }))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SigninResponse {
    pub token: String
}

#[post("/signin")]
async fn signin(app_state: web::Data<AppState>, body: web::Json<LoginRequest>) -> impl Responder {
    
    let result = app_state.db.get_user_by_username(&body.username).await;
    
    match result {
        Ok(user) => {
            let argon2 = Argon2::default();
            let parsed_hash = PasswordHash::new(&user.password);
            if let Ok(hash) = parsed_hash {
                if argon2.verify_password(body.password.as_bytes(), &hash).is_ok() {
                    let token = match create_jwt_for_user(&user.id.to_string(), 24) {
                        Ok(t) => t,
                        Err(_) => return HttpResponse::InternalServerError().finish(),
                    };
                    HttpResponse::Ok().json(SigninResponse { token })
                } else {
                    HttpResponse::Unauthorized().json(serde_json::json!({
                        "error": "invalid credentials"
                    }))
                }
            } else {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "failed to parse password hash"
                }))
            }
        }
        Err(_) => {
            HttpResponse::InternalServerError().json(serde_json::json!({
                "message": format!("username {} not found", body.username)
            }))
        }
    }
}

#[get("/me")]
async fn me(req: HttpRequest) -> impl Responder {
    if let Some(uid) = req.extensions().get::<Uuid>() {
        HttpResponse::Ok().json(serde_json::json!({
            "user_id": uid.to_string()
        }))
    } else {
        HttpResponse::Unauthorized().finish()
    }
}
