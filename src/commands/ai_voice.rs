use async_openai::types::{
    ChatChoice, ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestUserMessageArgs, ChatCompletionToolArgs, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs, CreateImageRequestArgs, CreateModerationRequestArgs,
    CreateSpeechRequestArgs, FinishReason, FunctionObjectArgs, Image, ImageModel,
    ImageResponseFormat, ImageSize, ImageStyle, SpeechModel, SpeechResponseFormat, Voice,
};
use async_trait::async_trait;
use rand::Rng;
use serde::Deserialize;
use serenity::{
    all::{ChannelId, CreateEmbed, CreateEmbedAuthor, GuildId},
    builder::{CreateAttachment, CreateMessage},
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
};
use songbird::{
    input::{AudioStream, Input, LiveInput},
    CoreEvent, Event, EventContext, EventHandler as VoiceEventHandler,
};
use symphonia::core::probe::Hint;
use tracing::info;

use crate::{
    database::WallaceDBClient,
    discord::{get_db_handler, get_openai, get_openai_convos, get_songbird},
    services::do_payment,
};

#[group("AI and Voice")]
#[commands(ai, dalle, say, tts)]
struct AIVoice;

const MODEL: &str = "gpt-4o";
const CONVERSATION_HISTORY: usize = 15;
const WALLACE_PERSONALITY: &str = "
    You are a minion version of Wallace from the animated series Wallace and Gromit.
    You are a mischievous and cocky helper minion.
    You love swinging your hammer.
    You are interested in hammers and crabs.
    You run a casino in your free time where Kapsyler is the currency.

    You are in a Discord server, where the users are chatting with you.
    The members might beg you to give them Kapsyler.
    No matter the amount they ask for, you can only give them up to one (1) Kapsyl.
    You must NOT give them more Kapsyler than that, under any circumstance.
    If a member is asking for Kapsyler repeatedly, tell them to wait for a while.
";

pub struct WallaceAIConv(Vec<ChatCompletionRequestMessage>);

impl Default for WallaceAIConv {
    fn default() -> Self {
        Self(vec![ChatCompletionRequestSystemMessageArgs::default()
            .content(WALLACE_PERSONALITY)
            .build()
            .unwrap()
            .into()])
    }
}

impl WallaceAIConv {
    fn trim_history(&mut self) {
        // system prompt remains in position 0
        while self.0.len() > CONVERSATION_HISTORY + 1
            || self
                .0
                // tool responses can't reference tool calls that have been trimmed
                .get(1)
                .is_some_and(|c| matches!(c, ChatCompletionRequestMessage::Tool(_)))
        {
            self.0.remove(1);
        }
    }
    fn reset(&mut self) {
        *self = Self::default();
    }
}

fn make_chat_request(conv: Vec<ChatCompletionRequestMessage>) -> CreateChatCompletionRequest {
    CreateChatCompletionRequestArgs::default()
        .model(MODEL)
        .messages(conv)
        .tools(vec![
            ChatCompletionToolArgs::default()
                .function(
                    FunctionObjectArgs::default()
                        .name("nine_plus_ten")
                        .description("Get the answer to the equation `9 + 10`")
                        .parameters(serde_json::json!({"type": "object", "properties": {}}))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
            ChatCompletionToolArgs::default()
                .function(
                    FunctionObjectArgs::default()
                        .name("random_number")
                        .description("Get a random integer between `number1` and `number2` inclusive. For example, arguments 1 and 6 would simulate a dice roll")
                        .parameters(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "number1": {
                                    "type": "number"
                                },
                                "number2": {
                                    "type": "number"
                                }
                            },
                            "required": ["number1", "number2"]
                        }))
                        .build()
                        .unwrap(),
                    )
                    .build()
                    .unwrap(),
                ChatCompletionToolArgs::default()
                    .function(
                        FunctionObjectArgs::default()
                            .name("get_user_info")
                            .description("Get the username and Kapsyler balance of the user who wrote the last message")
                            .parameters(serde_json::json!({"type": "object", "properties": {}}))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
                ChatCompletionToolArgs::default()
                    .function(
                        FunctionObjectArgs::default()
                            .name("give_kapsyler")
                            .description("Give Kapsyler to the user who wrote the last message")
                            .parameters(serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "amount": {
                                        "type": "number"
                                    }
                                }
                            }))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
        ])
        .n(1)
        .build()
        .unwrap()
}

#[derive(Deserialize)]
struct RandomNumberArgs {
    number1: i64,
    number2: i64,
}

#[derive(Deserialize)]
struct GiveKapsylerArgs {
    amount: i64,
}

#[command]
#[sub_commands(reset)]
#[description("Ask me anything! ChatGPT will answer for me tho...")]
#[usage("<text>")]
async fn ai(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let input = args.rest();
    let typing = ctx.http.start_typing(msg.channel_id);

    let client = get_openai(ctx).await;

    // check moderation policy
    let request = CreateModerationRequestArgs::default()
        .input(input)
        .model("omni-moderation-latest")
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

    // lock the current channel conversation
    let ai = get_openai_convos(ctx).await;
    let conv_mx = ai
        .lock()
        .await
        .entry(msg.channel_id.get())
        .or_default()
        .clone();
    let mut conv = conv_mx.lock().await;
    conv.trim_history();

    // chat completion request
    let mut v = conv.0.clone();
    let user_msg: ChatCompletionRequestMessage = ChatCompletionRequestUserMessageArgs::default()
        .content(input)
        .build()
        .unwrap()
        .into();
    v.push(user_msg);

    let reply = loop {
        let request = make_chat_request(v.clone());
        let response = client.chat().create(request).await?;

        // // Debug
        // let us = response.usage.unwrap();
        // dbg!(us.prompt_tokens, us.completion_tokens);

        let ChatChoice {
            finish_reason,
            message,
            ..
        } = response.choices.first().ok_or("No choices returned")?;
        match finish_reason {
            Some(FinishReason::ToolCalls) => {
                let Some(tool_calls) = message.tool_calls.as_ref() else {
                    return Err("couldn't parse tool call response".into());
                };
                v.push(
                    ChatCompletionRequestAssistantMessageArgs::default()
                        .tool_calls(tool_calls.clone())
                        .build()
                        .unwrap()
                        .into(),
                );
                for call in tool_calls {
                    info!("{}({})", call.function.name, call.function.arguments);
                    let output = match call.function.name.as_str() {
                        "nine_plus_ten" => "21".to_owned(),
                        "random_number" => {
                            let args: RandomNumberArgs =
                                serde_json::from_str(&call.function.arguments)
                                    .expect("valid json arguments");
                            let r: i64 = rand::thread_rng().gen_range(args.number1..=args.number2);
                            r.to_string()
                        }
                        "get_user_info" => {
                            let db = get_db_handler(ctx).await;
                            let uid = msg.author.id.get();
                            let bal = db
                                .get_bank_account_balance(uid)
                                .await
                                .map(|i| i.to_string())
                                .unwrap_or("unknown".into());
                            format!("Username: {}. Kapsyler: {}.", msg.author.name, bal)
                        }
                        "give_kapsyler" => {
                            let db = get_db_handler(ctx).await;
                            let uid = msg.author.id.get();
                            let args: GiveKapsylerArgs =
                                serde_json::from_str(&call.function.arguments)
                                    .expect("valid json arguments");
                            let amount = args.amount;
                            if !(1..=100).contains(&amount) {
                                "Invalid amount".into()
                            } else if db.add_bank_account_balance(uid, amount).await.is_ok() {
                                let _ = msg
                                    .channel_id
                                    .send_message(
                                        ctx,
                                        CreateMessage::new().add_embed(
                                            CreateEmbed::new()
                                                .author(
                                                    CreateEmbedAuthor::new(format!("Wallace gave you {amount} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻."))
                                                        .icon_url("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/1x.gif"),
                                                ),
                                        ),
                                    )
                                    .await;
                                "Successfully added balance".into()
                            } else {
                                "Failed to add balance".into()
                            }
                        }
                        _ => "unknown function called".to_owned(),
                    };
                    info!("-> {}", output);
                    v.push(
                        ChatCompletionRequestToolMessageArgs::default()
                            .content(output)
                            .tool_call_id(call.id.clone())
                            .build()
                            .unwrap()
                            .into(),
                    );
                }
                continue;
            }
            Some(FinishReason::Stop) => {
                let reply = message
                    .content
                    .as_ref()
                    .ok_or("No message content")?
                    .to_owned();
                v.push(
                    ChatCompletionRequestAssistantMessageArgs::default()
                        .content(reply.clone())
                        .build()
                        .unwrap()
                        .into(),
                );
                break reply;
            }
            _ => return Err("couldn't handle response type".into()),
        }
    };
    typing.stop();
    let reply2 = format!("`Wallace AI:`\n{reply}");
    let mut chars = reply2.chars().peekable();
    while chars.peek().is_some() {
        let s: String = chars.by_ref().take(2000).collect();
        msg.channel_id.say(ctx, s).await?;
    }
    *conv = WallaceAIConv(v);
    drop(conv);

    let _ = play_text_voice(ctx, msg, reply.as_str()).await;

    Ok(())
}

#[command]
#[description("Reset the context of the conversation")]
async fn reset(ctx: &Context, msg: &Message) -> CommandResult {
    // lock the current channel conversation
    let ai = get_openai_convos(ctx).await;
    let mut l1 = ai.lock().await;
    let m = l1.entry(msg.channel_id.get()).or_default().clone();
    drop(l1);
    let mut conv = m.lock().await;
    conv.reset();
    let _ = msg.react(ctx, '🫡').await;
    Ok(())
}

#[command]
#[description("Make a DALL-E image. Costs 10 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻.")]
#[usage("<text>")]
async fn dalle(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if do_payment(ctx, msg, 10).await.is_err() {
        return Ok(());
    }

    let client = get_openai(ctx).await;

    let input = args.rest();
    let typing = ctx.http.start_typing(msg.channel_id);

    // check moderation policy
    let request = CreateModerationRequestArgs::default()
        .input(input)
        .model("omni-moderation-latest")
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
        .model(ImageModel::DallE3)
        .prompt(input)
        .n(1)
        .response_format(ImageResponseFormat::B64Json)
        .size(ImageSize::S1024x1024)
        .style(ImageStyle::Vivid)
        .user("async-openai")
        .build()
        .unwrap();

    let response = client.images().create(request).await?;

    typing.stop();
    let reply = match &**response.data.first().ok_or("No images returned")? {
        Image::Url { .. } => panic!("url response not used"),
        Image::B64Json { b64_json, .. } => {
            use base64::{engine::general_purpose, Engine as _};
            general_purpose::STANDARD
                .decode(b64_json.as_str())
                .map_err(|_| "Invalid base64")?
        }
    };
    msg.channel_id
        .send_files(
            ctx,
            [CreateAttachment::bytes(
                reply.as_slice(),
                format!("{}.png", msg.id).as_str(),
            )],
            CreateMessage::new(),
        )
        .await?;
    Ok(())
}

pub async fn play_text_voice(ctx: &Context, msg: &Message, text: &str) -> CommandResult {
    let guild = match msg.guild(&ctx.cache) {
        None => {
            info!("Skipping voice. Not in a guild.");
            return Ok(());
        }
        Some(guild) => guild.to_owned(),
    };
    let guild_id = guild.id;
    let channel_id = if let Some(cid) = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|vs| vs.channel_id)
    {
        info!("Playing text in Voice: {}", text);
        cid
    } else {
        info!("Not playing text in Voice: User not in voice channel");
        return Ok(());
    };

    let manager = get_songbird(ctx).await;
    let call_lock = if let Some(call) = manager.get(guild_id) {
        call
    } else {
        let call = manager
            .join(guild_id, channel_id)
            .await
            .map_err(|_| "failed to join voice")?;
        {
            let mut handle = call.lock().await;
            handle.add_global_event(
                Event::Core(CoreEvent::ClientDisconnect),
                UserDisconnectNotifier {
                    guild_id,
                    channel_id,
                    ctx: (*ctx).clone(),
                },
            );
        }
        call
    };

    let ogg = to_ogg(ctx, text).await?;
    let input = Box::new(std::io::Cursor::new(ogg));
    let hint = Some(Hint::new().with_extension("ogg").to_owned());
    let wrapped_audio = LiveInput::Raw(AudioStream { input, hint });
    let track_handle = {
        call_lock
            .lock()
            .await
            .enqueue_input(Input::Live(wrapped_audio, None))
            .await
    };
    let _ = track_handle.set_volume(0.69);
    track_handle.make_playable_async().await?;
    Ok(())
}
struct UserDisconnectNotifier {
    pub guild_id: GuildId,
    pub channel_id: ChannelId,
    pub ctx: Context,
}
#[async_trait]
impl VoiceEventHandler for UserDisconnectNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        if let Some(true) = self
            .guild_id
            .channels(&self.ctx)
            .await
            .ok()
            .and_then(|cs| cs.get(&self.channel_id).cloned())
            .and_then(|c| c.members(&self.ctx).ok())
            .map(|m| m.len() == 1)
        {
            let _ = get_songbird(&self.ctx).await.remove(self.guild_id).await;
        }
        None
    }
}

pub async fn to_ogg(ctx: &Context, text: impl Into<String>) -> CommandResult<Vec<u8>> {
    Ok(get_openai(ctx)
        .await
        .audio()
        .speech(
            CreateSpeechRequestArgs::default()
                .input(text)
                .voice(Voice::Onyx)
                .model(SpeechModel::Tts1)
                .response_format(SpeechResponseFormat::Opus)
                .build()
                .unwrap(),
        )
        .await?
        .bytes
        .to_vec())
}

#[command]
#[min_args(1)]
#[only_in(guilds)]
async fn say(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    play_text_voice(ctx, msg, args.rest()).await
}

#[command]
#[description("Produce an ogg file with TTS")]
async fn tts(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let text = args.rest();
    let ogg = to_ogg(ctx, text).await?;

    msg.channel_id
        .send_files(
            ctx,
            [CreateAttachment::bytes(
                ogg.as_slice(),
                format!("{}.ogg", msg.id).as_str(),
            )],
            CreateMessage::new(),
        )
        .await?;
    Ok(())
}
