use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
    CreateChatCompletionRequestArgs, CreateImageRequestArgs, CreateModerationRequestArgs,
    ImageData, ImageSize, ResponseFormat, Role, TextModerationModel,
};
use async_trait::async_trait;
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

use crate::{
    discord::{get_openai, get_songbird},
    services::{
        do_payment, get_lang_flag,
        polly::{to_mp3, PollyLanguage},
    },
};

#[group("AI and Voice")]
#[commands(ai, dalle, say, tts, languages)]
struct AIVoice;

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
    fn add(&mut self, prompt: ChatCompletionRequestMessage, reply: ChatCompletionRequestMessage) {
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
async fn ai(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let lang = get_lang_flag(&mut args);
    let input = args.rest();
    let typing = ctx.http.start_typing(msg.channel_id);

    // lock the current channel conversation
    let ai = get_openai(ctx).await;
    let mut l1 = ai.lock().await;
    let client = l1.0.clone();
    let m = l1.1.entry(msg.channel_id.0.get()).or_default().clone();
    drop(l1);

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

    let mut conv = m.lock().await; // hold lock until end of command
    conv.trim_history();

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

    let _ = play_text_voice(ctx, msg, reply, lang).await;

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
        .response_format(ResponseFormat::Url)
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
            general_purpose::STANDARD
                .decode(b64.as_str())
                .map_err(|_| "Invalid base64")?
        }
    };
    msg.channel_id
        .send_files(
            ctx,
            [CreateAttachment::bytes(
                reply.as_slice(),
                format!("{}.png", msg.id.0).as_str(),
            )],
            CreateMessage::new(),
        )
        .await?;
    Ok(())
}

pub async fn play_text_voice(
    ctx: &Context,
    msg: &Message,
    text: &str,
    lang: Option<PollyLanguage>,
) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap().to_owned();
    let guild_id = guild.id;
    let channel_id = if let Some(cid) = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|vs| vs.channel_id)
    {
        cid
    } else {
        return Ok(()); // User not in voice channel
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

    let sound_data = to_mp3(ctx, text, lang).await?;
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

#[command]
#[min_args(1)]
#[only_in(guilds)]
async fn say(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let lang = get_lang_flag(&mut args);
    play_text_voice(ctx, msg, args.rest(), lang).await
}

#[command]
#[description("Produce an mp3 with TTS")]
async fn tts(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let lang = get_lang_flag(&mut args);
    let text = args.rest();

    let mp3 = to_mp3(ctx, text, lang).await?;

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
#[description("Show all voice languages supported and their identifier")]
async fn languages(ctx: &Context, msg: &Message) -> CommandResult {
    let _ = msg
        .channel_id
        .say(ctx, PollyLanguage::to_list_string())
        .await;
    Ok(())
}
