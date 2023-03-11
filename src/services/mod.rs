pub mod cool_text;
pub mod polly;
pub mod riot_api;
pub mod seven_tv;

use std::num::NonZeroU64;

use anyhow::anyhow;
use chrono::Duration;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serenity::{
    builder::{CreateEmbed, CreateEmbedAuthor, CreateMessage, EditGuild, EditMember},
    client::Context,
    framework::standard::{Args, CommandResult},
    model::prelude::{Guild, Message, Timestamp, UserId},
};

use cool_text::{to_cool_text, Font};

use crate::{database::WallaceDBClient, discord::get_db_handler};

use self::polly::PollyLanguage;

pub async fn set_server_name<'a>(
    ctx: &Context,
    mut guild: Guild,
    reply_to: Option<&Message>,
    name: &str,
) -> CommandResult {
    guild.edit(ctx, EditGuild::new().name(name)).await?;
    if let Some(msg) = reply_to {
        msg.channel_id
            .say(ctx, format!("Set server name to '{}'", name))
            .await?;
    }
    Ok(())
}

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
    "https://cdn.7tv.app/emote/61e0be4f4f44b95f34661a89/2x.gif",
    "https://cdn.7tv.app/emote/60ee6dbf77c3ca347fdfea8b/2x.gif",
    "https://cdn.7tv.app/emote/6116999a7327a61fe25e580c/2x.gif",
    "https://cdn.7tv.app/emote/61bcc6e25804e220aa6adc77/2x.gif",
    "https://cdn.7tv.app/emote/62734c1ade98b688d09661d6/2x.gif",
];
pub async fn bonk_user(ctx: &Context, msg: &Message, uid: u64, duration: u32) -> CommandResult {
    let gid = msg.guild_id.ok_or("Failed to get guild")?;
    if let Err(e) = gid
        .edit_member(
            ctx,
            UserId(NonZeroU64::new(uid).expect("not 0")),
            EditMember::new().disable_communication_until(
                Timestamp::now()
                    .checked_add_signed(Duration::seconds(duration as i64))
                    .expect("Failed to add date")
                    .to_rfc3339(),
            ),
        )
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
    let mut rng: StdRng = SeedableRng::from_entropy();
    let tn = gid
        .member(ctx, uid)
        .await
        .map(|m| m.nick.unwrap_or(m.user.name))
        .unwrap_or_else(|_| "?".into());
    let _ = msg
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .author(CreateEmbedAuthor::new(format!(
                        "{}🔨🙂",
                        to_cool_text("BONK!", Font::BoldScript)
                    )))
                    .title(format!("Timed out {tn} for {duration} seconds."))
                    .thumbnail(BONK_EMOTES[rng.gen_range(0..BONK_EMOTES.len())]),
            ),
        )
        .await;
    Ok(())
}

pub async fn unbonk_user(ctx: &Context, msg: &Message, uid: u64) -> CommandResult {
    let gid = msg.guild_id.ok_or("Failed to get guild")?;
    if let Err(e) = gid
        .edit_member(
            ctx,
            UserId(NonZeroU64::new(uid).expect("not 0")),
            EditMember::new().enable_communication(),
        )
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
    let tn = gid
        .member(ctx, uid)
        .await
        .map(|m| m.nick.unwrap_or(m.user.name))
        .unwrap_or_else(|_| "?".into());
    let _ = msg
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .author(CreateEmbedAuthor::new(format!(
                        "{}🔨🙂",
                        to_cool_text("UNBONK!", Font::BoldScript)
                    )))
                    .title(format!("{tn} is free now."))
                    .thumbnail("https://cdn.7tv.app/emote/60ed8bf39dd7fe3b46c62e0f/2x.gif"),
            ),
        )
        .await;
    Ok(())
}

pub async fn nickname_user(ctx: &Context, msg: &Message, uid: u64, nick: String) -> CommandResult {
    let gid = msg.guild_id.ok_or("Failed to get guild")?;
    if let Err(e) = gid
        .edit_member(
            ctx,
            UserId(NonZeroU64::new(uid).expect("not 0")),
            EditMember::new().nickname(nick),
        )
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
    let _ = msg.react(ctx, '🫡').await;
    Ok(())
}

pub async fn do_payment(ctx: &Context, msg: &Message, amount: i64) -> CommandResult {
    let db = get_db_handler(ctx).await;
    if let Err(e) = db
        .subtract_bank_account_balance(msg.author.id.0.get(), amount)
        .await
    {
        let _ = msg.channel_id.say(ctx, e.to_string()).await;
        Err(anyhow!("").into())
    } else {
        let _ = msg
            .channel_id
            .send_message(
                ctx,
                CreateMessage::new().add_embed(
                    CreateEmbed::new().author(
                        CreateEmbedAuthor::new(format!("-{amount} 𝓚"))
                            .icon_url("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/1x.gif"),
                    ),
                ),
            )
            .await;
        Ok(())
    }
}

pub fn get_lang_flag(args: &mut Args) -> Option<PollyLanguage> {
    if args.current().is_none() {
        return None;
    };
    let a = args.current().unwrap().to_owned();
    if a.starts_with('-') {
        args.advance();
        a.strip_prefix('-').unwrap().parse::<PollyLanguage>().ok()
    } else {
        None
    }
}
