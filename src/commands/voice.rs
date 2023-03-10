use async_trait::async_trait;
use serenity::{
    all::{ChannelId, GuildId},
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

use crate::{discord::get_songbird, services::polly::to_mp3};

#[group]
#[commands(play)]
struct Voice;

pub async fn play_text_voice(ctx: &Context, msg: &Message, text: &str) -> CommandResult {
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
    let _ = track_handle.set_volume(0.8);
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
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    play_text_voice(ctx, msg, args.rest()).await
}
