use sqlx::{PgPool, postgres::PgPoolOptions};
use anyhow::Result; 

pub mod models;

#[derive(Clone)]
pub struct Db {
    pool: PgPool
}

impl Db {
    pub async fn new() -> Result<Self> {
        let url = std::env::var("DATABASE_URL").expect("DATABSE_URL must be set in .env file");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url).await?;
        Ok(Self {
            pool
        })
    }
    
}
