use shuttle_secrets::{SecretStore, Secrets};
use shuttle_service::ShuttleSerenity;

use discord::build_bot;

mod commands;
mod database;
mod discord;
mod prisma;
mod services;

#[shuttle_service::main]
async fn serenity(#[Secrets] secret_store: SecretStore) -> ShuttleSerenity {
    // Get the tokens set in `Secrets[.dev].toml`
    let discord_token = secret_store
        .get("DISCORD_TOKEN")
        .expect("Discord token missing! (env variable `DISCORD_TOKEN`)");
    let riot_token_lol = secret_store
        .get("RIOT_TOKEN_LOL")
        .expect("Riot token for LoL missing! (env variable `RIOT_TOKEN_LOL`)");
    let riot_token_tft = secret_store
        .get("RIOT_TOKEN_TFT")
        .expect("Riot token for TFT missing! (env variable `RIOT_TOKEN_TFT`)");
    let db_url = secret_store
        .get("DATABASE_URL")
        .expect("URL for database missing! (env variable `DATABASE_URL`)");

    let client = build_bot(discord_token, riot_token_lol, riot_token_tft, db_url).await;

    Ok(client)
}
