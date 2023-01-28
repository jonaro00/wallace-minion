pub mod cool_text;
pub mod riot_api;
pub mod seven_tv;

use rand::{rngs::StdRng, Rng, SeedableRng};
use serenity::{
    client::Context,
    framework::standard::CommandResult,
    model::prelude::{Message, Timestamp, UserId},
};
use time::{format_description::well_known::Iso8601, Duration};

use cool_text::{to_cool_text, Font};

const TIMEOUT_SECS: i64 = 60;
const BONK_EMOTES: &[&str] = &[
    "https://cdn.7tv.app/emote/631b61a98cf0978e2955b04f/2x.gif",
    "https://cdn.7tv.app/emote/60d174a626215e098873e43e/2x.gif",
    "https://cdn.7tv.app/emote/60b92f83db8410b2f367d817/2x.gif",
    "https://cdn.7tv.app/emote/60aea79d4b1ea4526d9b20a9/2x.gif",
    "https://cdn.7tv.app/emote/610d5af489f87ba0b7040211/2x.gif",
    "https://cdn.7tv.app/emote/60afa73da3648f409ad99659/2x.gif",
    "https://cdn.7tv.app/emote/6214fb58e82b29ebce528953/2x.gif",
    "https://cdn.7tv.app/emote/60ecac47bb9d6a49d7d7d46e/2x.gif",
    "https://cdn.7tv.app/emote/63444f53df8d9e6ac32ab703/2x.gif",
    "https://cdn.7tv.app/emote/62ae151c260bc8642e64ec36/2x.gif",
    "https://cdn.7tv.app/emote/603eacea115b55000d7282dc/2x.gif",
    "https://cdn.7tv.app/emote/62734c1ade98b688d09661d6/2x.gif",
];
pub async fn bonk_user(ctx: &Context, msg: &Message, uid: u64) -> CommandResult {
    let gid = msg.guild_id.ok_or("Failed to get guild")?;
    if let Err(e) = gid
        .edit_member(ctx, UserId(uid), |m| {
            m.disable_communication_until(
                Timestamp::now()
                    .checked_add(Duration::seconds(TIMEOUT_SECS))
                    .expect("Failed to add date")
                    .format(&Iso8601::DEFAULT)
                    .expect("Failed to format date"),
            )
        })
        .await
    {
        let s = e.to_string();
        let _ = msg
            .channel_id
            .say(
                ctx,
                if s == "Missing Permissions" {
                    "That guy is too powerful... I can't do it... 😔".into()
                } else {
                    s
                },
            )
            .await;
        return Ok(());
    };
    let _ = msg
        .channel_id
        .say(
            ctx,
            format!(
                "{}🔨🙂 Timed out <@{}> for {} seconds.",
                to_cool_text("BONK!", Font::BoldScript),
                uid,
                TIMEOUT_SECS,
            ),
        )
        .await;
    let mut rng: StdRng = SeedableRng::from_entropy();
    let _ = msg
        .channel_id
        .say(ctx, BONK_EMOTES[rng.gen_range(0..BONK_EMOTES.len())])
        .await;
    Ok(())
}
