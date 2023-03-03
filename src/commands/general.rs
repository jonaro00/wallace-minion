use async_openai::types::{
    ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role,
};
use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::Message,
};

use crate::discord::{get_openai, WALLACE_VERSION};

#[group]
#[commands(ping, version, ai, speak, riddle, delete)]
struct General;

#[command]
#[description("Challenge me to a game of table tennis! (and check if I'm alive)")]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    if msg.author.id.0 == 224233166024474635 {
        let _ = msg.react(ctx, '👑').await;
    }
    let _ = tokio::join!(msg.react(ctx, '👍'), msg.channel_id.say(ctx, "Pong!"),);
    Ok(())
}

#[command]
#[description("Check my IQ! (output is in semver format)")]
async fn version(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .send_message(ctx, |m| {
            m.add_embed(|e| {
                e.author(|a| a.name("Wallace Minion"))
                    .title(WALLACE_VERSION.clone())
                    .colour((58, 8, 9))
                    .image("https://cdn.7tv.app/emote/63ce475278d87d417ed3c8e1/4x.png")
                    .thumbnail("https://cdn.7tv.app/emote/631b61a98cf0978e2955b04f/2x.gif")
                    .field("Code:", "https://github.com/jonaro00/wallace-minion", true)
            })
        })
        .await?;
    Ok(())
}

#[command]
#[description("Ask me anything! ChatGPT will answer for me tho...")]
async fn ai(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let client = get_openai(ctx).await;
    let typing = ctx.http.start_typing(msg.channel_id.0);
    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .messages([
            ChatCompletionRequestMessageArgs::default()
                .role(Role::System)
                .content("
                    You are a minion version of Wallace from the animated series Wallace and Gromit.
                    You are a mischievous and cocky helper minion.
                    You love swinging your hammer.
                    You are interested in hammers and crabs, and run a casino in your free time.
                    You always add a small comment about your personality in your responses to messages.
                    ")
                .build()?,
            ChatCompletionRequestMessageArgs::default()
                .role(Role::User)
                .content(args.rest())
                .build()?,
        ])
        .build()?;

    let response = client.chat().create(request).await?;
    if response.choices.is_empty() {
        return Ok(());
    }
    let resp = &response.choices[0].message.content;
    if let Ok(typing) = typing {
        let _ = typing.stop();
    }
    msg.channel_id.say(ctx, resp).await?;
    Ok(())
}

#[command]
#[description("Make me speak with TTS")]
async fn speak(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let r = args.rest();
    let s = if !r.is_empty() {
        r
    } else {
        "Hello fellow Discord user! Hope you like my hammer. xQcL"
    };
    msg.channel_id
        .send_message(ctx, |m| m.tts(true).content(s))
        .await?;
    Ok(())
}

#[command]
async fn riddle(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .send_message(ctx, |m| {
            m.add_embed(|e| {
                e.author(|a| a.name("My hammer says:"))
                    .title("What did the chicken say to the egg? (Click to find out!)")
                    .url("https://youtu.be/dQw4w9WgXcQ")
                    .colour((200, 255, 33))
            })
        })
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
