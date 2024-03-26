use std::net::SocketAddr;

use async_trait::async_trait;
use shuttle_runtime::{SecretStore, Secrets};

use discord::build_bot;

mod commands;
mod database;
mod discord;
mod model;
mod services;

#[shuttle_runtime::main]
async fn serenity(#[Secrets] secrets: SecretStore) -> Result<MyService, shuttle_runtime::Error> {
    // Get the tokens set in `Secrets[.dev].toml`
    let discord_token = secrets
        .get("DISCORD_TOKEN")
        .expect("Discord token missing! (env variable `DISCORD_TOKEN`)");
    let riot_token_lol = secrets
        .get("RIOT_TOKEN_LOL")
        .expect("Riot token for LoL missing! (env variable `RIOT_TOKEN_LOL`)");
    let riot_token_tft = secrets
        .get("RIOT_TOKEN_TFT")
        .expect("Riot token for TFT missing! (env variable `RIOT_TOKEN_TFT`)");
    let db_url = secrets
        .get("DATABASE_URL")
        .expect("URL for database missing! (env variable `DATABASE_URL`)");
    let openai_token = secrets
        .get("OPENAI_TOKEN")
        .expect("OpenAI token missing! (env variable `OPENAI_TOKEN`)");

    let client = build_bot(
        discord_token,
        riot_token_lol,
        riot_token_tft,
        db_url,
        openai_token,
    )
    .await;

    Ok(MyService {
        discord: client,
        // other: (),
    })
}

struct MyService {
    pub discord: serenity::Client,
    // pub other: (),
}
#[async_trait]
impl shuttle_runtime::Service for MyService {
    async fn bind(mut self, _addr: SocketAddr) -> Result<(), shuttle_runtime::Error> {
        self.discord
            .start()
            .await
            .map_err(shuttle_runtime::CustomError::new)?;

        Ok(())
    }
}
