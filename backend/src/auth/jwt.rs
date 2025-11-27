use anyhow::Result;
use chrono::{Utc, Duration};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey, TokenData};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize
}

fn secret() -> String {
    std::env::var("JWT_SECRET").expect("JWT_SECRET must be set")
}

pub fn create_jwt_for_user(user_id: &str, hours_valid: i64) -> Result<String> {
    let exp = (Utc::now() + Duration::hours(hours_valid)).timestamp() as usize;
    let claims = Claims { sub: user_id.to_owned(), exp };
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret().as_ref()))?;
    Ok(token)  
}

pub fn verify_jwt(token: &str) -> Result<TokenData<Claims>> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret().as_ref()),
        &Validation::default()
    )?;
    Ok(token_data)
}