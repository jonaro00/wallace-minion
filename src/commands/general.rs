use serenity::{
    builder::{CreateEmbed, CreateEmbedAuthor, CreateMessage},
    client::Context,
    framework::standard::{
        macros::{command, group},
        CommandResult,
    },
    model::prelude::Message,
};

use crate::discord::WALLACE_VERSION;

#[group]
#[commands(ping, version, riddle, delete)]
struct General;

#[command]
#[description("Challenge me to a game of table tennis! (and check if I'm alive)")]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    if msg.author.id.0.get() == 224233166024474635 {
        let _ = msg.react(ctx, '👑').await;
    }
    let _ = tokio::join!(msg.react(ctx, '👍'), msg.channel_id.say(ctx, "Pong!"),);
    Ok(())
}

#[command]
#[description("Check my IQ! (output is in semver format)")]
async fn version(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .author(CreateEmbedAuthor::new("Wallace Minion"))
                    .title(WALLACE_VERSION.clone())
                    .colour((58, 8, 9))
                    .image("https://cdn.7tv.app/emote/63ce475278d87d417ed3c8e1/4x.png")
                    .thumbnail("https://cdn.7tv.app/emote/631b61a98cf0978e2955b04f/2x.gif")
                    .field("Code:", "https://github.com/jonaro00/wallace-minion", true),
            ),
        )
        .await?;
    Ok(())
}

#[command]
async fn riddle(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .author(CreateEmbedAuthor::new("My hammer says:"))
                    .title("What did the chicken say to the egg? (Click to find out!)")
                    .url("https://youtu.be/dQw4w9WgXcQ")
                    .colour((200, 255, 33)),
            ),
        )
        .await?;
    Ok(())
}

#[command]
#[description("Censor me. Reply to a message from me with this command to delete it.")]
async fn delete(ctx: &Context, msg: &Message) -> CommandResult {
    let _ = msg.delete(ctx).await;
    if let Some(r) = msg.referenced_message.as_deref() {
        if r.author.id == ctx.cache.current_user().id {
            r.delete(ctx).await?;
        }
    }
    Ok(())
}
