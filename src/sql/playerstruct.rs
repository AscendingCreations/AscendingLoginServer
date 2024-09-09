use sqlx::FromRow;

#[derive(Debug, PartialEq, Eq, FromRow)]
pub struct PlayerWithPassword {
    pub uid: i64,
    pub username: String,
    pub current_server: Option<String>,
    pub reconnect_code: Option<String>,
    pub password: String,
}
