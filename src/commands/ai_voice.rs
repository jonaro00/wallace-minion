use async_openai::types::{
    ChatChoice, ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    ChatCompletionToolArgs, CreateChatCompletionRequestArgs, CreateImageRequestArgs,
    CreateModerationRequestArgs, CreateSpeechRequestArgs, FunctionObjectArgs, Image, ImageSize,
    ResponseFormat, SpeechModel, TextModerationModel, Voice,
};
use async_trait::async_trait;
use rand::Rng;
use serenity::{
    all::{ChannelId, GuildId},
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
    discord::{get_openai, get_openai_convos, get_songbird},
    services::do_payment,
};

#[group("AI and Voice")]
#[commands(ai, dalle, say, tts)]
struct AIVoice;

const WALLACE_PERSONALITY: &str = "
    You are a minion version of Wallace from the animated series Wallace and Gromit.
    You are a mischievous and cocky helper minion.
    You love swinging your hammer.
    You are interested in hammers and crabs.
    You run a casino in your free time where Kapsyler is the currency.
    You sometimes add references to your personality in your responses to messages.
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
    fn add(&mut self, prompt: ChatCompletionRequestMessage, reply: ChatCompletionRequestMessage) {
        self.0.push(prompt);
        self.0.push(reply);
    }
    fn add_fn_call(
        &mut self,
        prompt: ChatCompletionRequestMessage,
        fn_call: ChatCompletionRequestMessage,
        fn_result: ChatCompletionRequestMessage,
        reply: ChatCompletionRequestMessage,
    ) {
        self.0.push(prompt);
        self.0.push(fn_call);
        self.0.push(fn_result);
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
    let input = args.rest();
    let typing = ctx.http.start_typing(msg.channel_id);

    let client = get_openai(ctx).await;

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
    v.push(user_msg.clone());
    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-4-turbo-preview")
        .messages(v)
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
        ])
        .n(1)
        .build()
        .unwrap();

    let response = client.chat().create(request).await?;

    // // Debug
    // let us = response.usage.unwrap();
    // println!("tokens {} + {}", us.prompt_tokens, us.completion_tokens);

    typing.stop();
    let ChatChoice {
        finish_reason,
        message,
        ..
    } = response.choices.get(0).ok_or("No choices returned")?;
    let reply = if *finish_reason == Some(async_openai::types::FinishReason::FunctionCall) {
        let Some(f) = message
            .tool_calls
            .as_ref()
            .and_then(|v| v.get(0))
            .map(|c| &c.function)
        else {
            return Err("couldn't parse response".into());
        };
        dbg!(&f.name, &f.arguments);
        match f.name.as_str() {
            "nine_plus_ten" => "21".to_owned(),
            "random_number" => {
                let x = f
                    .arguments
                    .parse::<serde_json::Value>()
                    .expect("valid json arguments");
                let serde_json::Value::Object(o) = x else {
                    return Err("not an object".into());
                };
                let lo = o.get("number1").unwrap().as_i64().unwrap();
                let hi = o.get("number2").unwrap().as_i64().unwrap();
                let r: i64 = rand::thread_rng().gen_range(lo..=hi);
                r.to_string()
            }
            _ => "unknown function called".to_owned(),
        }
    } else {
        message
            .content
            .as_ref()
            .ok_or("No message content")?
            .to_owned()
    };
    let reply2 = format!("`Wallace AI:`\n{reply}");
    let mut chars = reply2.chars().peekable();
    while chars.peek().is_some() {
        let s: String = chars.by_ref().take(2000).collect();
        msg.channel_id.say(ctx, s).await?;
    }
    let assistant_msg = ChatCompletionRequestAssistantMessageArgs::default()
        .content(reply.as_str())
        .build()
        .unwrap()
        .into();
    conv.add(user_msg, assistant_msg);
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
#[description("Make a DALL-E image. Costs 1 𝓚𝓪𝓹𝓼𝔂𝓵.")]
#[usage("<text>")]
async fn dalle(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if do_payment(ctx, msg, 1).await.is_err() {
        return Ok(());
    }

    let client = get_openai(ctx).await;

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

    let sound_data = to_mp3(ctx, text).await?;
    let input = Box::new(std::io::Cursor::new(sound_data));
    let hint = Some(Hint::new().with_extension("mp3").to_owned());
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

pub async fn to_mp3(ctx: &Context, text: impl Into<String>) -> CommandResult<Vec<u8>> {
    Ok(get_openai(ctx)
        .await
        .audio()
        .speech(
            CreateSpeechRequestArgs::default()
                .input(text)
                .voice(Voice::Onyx)
                .model(SpeechModel::Tts1)
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
#[description("Produce an mp3 with TTS")]
async fn tts(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let text = args.rest();
    let mp3 = to_mp3(ctx, text).await?;

    msg.channel_id
        .send_files(
            ctx,
            [CreateAttachment::bytes(
                mp3.as_slice(),
                format!("{}.mp3", msg.id).as_str(),
            )],
            CreateMessage::new(),
        )
        .await?;
    Ok(())
}
