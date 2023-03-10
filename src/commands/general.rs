use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
    CreateChatCompletionRequestArgs, CreateImageRequestArgs, CreateModerationRequestArgs,
    ImageData, ImageSize, ResponseFormat, Role, TextModerationModel,
};
use serenity::{
    builder::{CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateMessage},
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::Message,
};

use crate::{
    commands::voice::play_text_voice,
    discord::{get_openai, WALLACE_VERSION},
    services::{do_payment, polly::to_mp3},
};

#[group]
#[commands(ping, version, ai, dalle, speak, riddle, delete)]
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

const WALLACE_PERSONALITY: &str = "
    You are a minion version of Wallace from the animated series Wallace and Gromit.
    You are a mischievous and cocky helper minion.
    You love swinging your hammer.
    You are interested in hammers and crabs.
    You run a casino in your free time where Kapsyler is the currency.
    You always add a small comment about your personality in your responses to messages.
";
pub struct WallaceAIConv(Vec<ChatCompletionRequestMessage>);
impl Default for WallaceAIConv {
    fn default() -> Self {
        Self(vec![ChatCompletionRequestMessageArgs::default()
            .role(Role::System)
            .content(WALLACE_PERSONALITY)
            .build()
            .unwrap()])
    }
}
impl WallaceAIConv {
    fn add(
        &mut self,
        prompt: ChatCompletionRequestMessage,
        reply: ChatCompletionRequestMessage,
    ) -> () {
        self.0.push(prompt);
        self.0.push(reply);
    }
    fn trim_history(&mut self) {
        while self.0.len() > 11 {
            self.0.remove(1);
        }
    }
    fn reset(&mut self) {
        *self = Self::default();
    }
}
#[command]
#[sub_commands(reset)]
#[description("Ask me anything! ChatGPT will answer for me tho...")]
#[usage("<text>")]
async fn ai(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // lock the current channel conversation
    let ai = get_openai(ctx).await;
    let mut l1 = ai.lock().await;
    let client = l1.0.clone();
    let m = l1.1.entry(msg.channel_id.0.get()).or_default().clone();
    drop(l1);
    let mut conv = m.lock().await;
    conv.trim_history();

    let input = args.rest();
    let typing = ctx.http.start_typing(msg.channel_id);

    // check moderation policy
    let request = CreateModerationRequestArgs::default()
        .input(input)
        .model(TextModerationModel::Latest)
        .build()
        .unwrap();
    let response = client.moderations().create(request).await?;
    if response.results[0].flagged {
        let _ = msg
            .channel_id
            .say(
                ctx,
                "❌ This prompt was flagged breaking OpenAI's content policy.",
            )
            .await;
        return Ok(());
    }

    // chat completion request
    let mut v = conv.0.clone();
    let user_msg = ChatCompletionRequestMessageArgs::default()
        .role(Role::User)
        .content(input)
        .build()
        .unwrap();
    v.push(user_msg.clone());
    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .messages(v)
        .build()
        .unwrap();

    let response = client.chat().create(request).await?;

    // // Debug
    // let us = response.usage.unwrap();
    // println!("tokens {} + {}", us.prompt_tokens, us.completion_tokens);

    typing.stop();
    let reply = response
        .choices
        .get(0)
        .ok_or("No choices returned")?
        .message
        .content
        .as_str();
    let s: String = format!("`Wallace AI:`\n{reply}")
        .chars()
        .take(2000)
        .collect();
    msg.channel_id.say(ctx, s).await?;
    let assistant_msg = ChatCompletionRequestMessageArgs::default()
        .role(Role::Assistant)
        .content(reply)
        .build()
        .unwrap();
    conv.add(user_msg, assistant_msg);

    let _ = play_text_voice(ctx, msg, reply).await;

    Ok(())
}

#[command]
#[description("Reset the context of the conversation")]
async fn reset(ctx: &Context, msg: &Message) -> CommandResult {
    // lock the current channel conversation
    let ai = get_openai(ctx).await;
    let mut l1 = ai.lock().await;
    let m = l1.1.entry(msg.channel_id.0.get()).or_default().clone();
    drop(l1);
    let mut conv = m.lock().await;
    conv.reset();
    let _ = msg.react(ctx, '🫡').await;
    Ok(())
}

#[command]
#[description("Make a DALL-E image. Costs 1 𝓚𝓪𝓹𝓼𝔂𝓵.")]
#[usage("<text>")]
async fn dalle(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if do_payment(ctx, msg, 1).await.is_err() {
        return Ok(());
    }

    let ai = get_openai(ctx).await;
    let l1 = ai.lock().await;
    let client = l1.0.clone();
    drop(l1);

    let input = args.rest();
    let typing = ctx.http.start_typing(msg.channel_id);

    // check moderation policy
    let request = CreateModerationRequestArgs::default()
        .input(input)
        .model(TextModerationModel::Latest)
        .build()
        .unwrap();
    let response = client.moderations().create(request).await?;
    if response.results[0].flagged {
        let _ = msg
            .channel_id
            .say(
                ctx,
                "❌ This prompt was flagged breaking OpenAI's content policy.",
            )
            .await;
        return Ok(());
    }

    // chat completion request
    let request = CreateImageRequestArgs::default()
        .prompt(input)
        .n(1)
        .response_format(ResponseFormat::B64Json)
        .size(ImageSize::S512x512)
        .user("async-openai")
        .build()
        .unwrap();

    let response = client.images().create(request).await?;

    typing.stop();
    let reply = match &**response.data.get(0).ok_or("No images returned")? {
        ImageData::Url(_) => panic!("url response not used"),
        ImageData::B64Json(b64) => {
            use base64::{engine::general_purpose, Engine as _};
            let v = general_purpose::STANDARD
                .decode(b64.as_str())
                .map_err(|_| "Invalid base64")?;
            v
        }
    };
    msg.channel_id
        .send_files(
            ctx,
            [(reply.as_slice(), format!("{}.png", msg.id.0).as_str())],
            |m| m,
        )
        .await?;
    Ok(())
}

#[command]
#[description("Make me speak with TTS")]
async fn speak(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let text = args.rest();

    let mp3 = to_mp3(ctx, text).await?;

    msg.channel_id
        .send_files(
            ctx,
            [CreateAttachment::bytes(
                mp3.as_slice(),
                format!("{}.mp3", msg.id.0).as_str(),
            )],
            CreateMessage::new(),
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
