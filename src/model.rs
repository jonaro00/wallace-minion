#[derive(sqlx::FromRow)]
pub struct LoLAccount {
    pub server: String,
    pub name: String,
    pub tag: String,
}

#[derive(sqlx::FromRow)]
pub struct Task {
    pub id: i32,
    pub cron: String,
    pub cmd: String,
    pub arg: Option<String>,
    pub channel_id: i64,
}

#[derive(sqlx::FromRow)]
pub struct User {
    pub id: i64,
    #[allow(unused)]
    pub mature: bool,
}
