use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
    CreateChatCompletionRequestArgs, CreateModerationRequestArgs, Role, TextModerationModel,
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

pub struct WallaceAIConv(Vec<ChatCompletionRequestMessage>);
impl Default for WallaceAIConv {
    fn default() -> Self {
        Self(vec![ChatCompletionRequestMessageArgs::default()
            .role(Role::System)
            .content(
                "
                You are a minion version of Wallace from the animated series Wallace and Gromit.
                You are a mischievous and cocky helper minion.
                You love swinging your hammer.
                You are interested in hammers and crabs, and run a casino in your free time.
                You always add a small comment about your personality in your responses to messages.
                ",
            )
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
    fn _reset(&mut self) {
        *self = Self::default();
    }
}
#[command]
#[description("Ask me anything! ChatGPT will answer for me tho...")]
#[usage("<text>")]
async fn ai(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // lock the current channel conversation
    let ai = get_openai(ctx).await;
    let mut l1 = ai.lock().await;
    let client = l1.0.clone();
    let m = l1.1.entry(msg.channel_id.0).or_default().clone();
    drop(l1);
    let mut conv = m.lock().await;
    conv.trim_history();

    let input = args.rest();
    let typing = ctx.http.start_typing(msg.channel_id.0);

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

    if response.choices.is_empty() {
        return Ok(());
    }
    if let Ok(typing) = typing {
        let _ = typing.stop();
    }
    let reply = &response
        .choices
        .get(0)
        .ok_or("No choices returned")?
        .message
        .content;
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
