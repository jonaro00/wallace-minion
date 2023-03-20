use shuttle_secrets::{SecretStore, Secrets};
use shuttle_serenity::{ShuttleSerenity, SerenityService};

use discord::build_bot;

mod commands;
mod database;
mod discord;
mod prisma;
mod services;

#[shuttle_runtime::main]
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
    let openai_token = secret_store
        .get("OPENAI_TOKEN")
        .expect("OpenAI token missing! (env variable `OPENAI_TOKEN`)");
    let aws_key_id = secret_store
        .get("AWS_ACCESS_KEY_ID")
        .expect("AWS Access Key Id missing! (env variable `AWS_ACCESS_KEY_ID`)");
    let aws_secret = secret_store
        .get("AWS_SECRET_ACCESS_KEY")
        .expect("AWS Secret Access Key missing! (env variable `AWS_SECRET_ACCESS_KEY`)");
    let aws_region = secret_store
        .get("AWS_REGION")
        .expect("AWS Region missing! (env variable `AWS_REGION`)");

    // To be consumed by `aws_config::load_from_env()`
    std::env::set_var("AWS_ACCESS_KEY_ID", aws_key_id);
    std::env::set_var("AWS_SECRET_ACCESS_KEY", aws_secret);
    std::env::set_var("AWS_REGION", aws_region);

    let client = build_bot(
        discord_token,
        riot_token_lol,
        riot_token_tft,
        db_url,
        openai_token,
    )
    .await;

    Ok(SerenityService(client))
}
