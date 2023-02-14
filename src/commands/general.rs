use rand::{rngs::StdRng, Rng, SeedableRng};
use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::Message,
    utils::parse_username,
};

use crate::{
    database::WallaceDBClient,
    discord::{get_db_handler, wallace_version},
    services::{bonk_user, set_server_name},
};

#[group]
#[commands(ping, version, speak, riddle, delete, bonk, defaultname, randomname)]
#[description("Test")]
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
                    .title(wallace_version())
                    .colour((58, 8, 9))
                    .image("https://cdn.7tv.app/emote/63ce475278d87d417ed3c8e1/4x.png")
                    .thumbnail("https://cdn.7tv.app/emote/631b61a98cf0978e2955b04f/2x.gif")
            })
        })
        .await?;
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

#[command]
#[aliases(hammer, timeout)]
#[only_in(guilds)]
#[min_args(1)]
#[required_permissions("ADMINISTRATOR")]
#[description("Bonk a user.")]
async fn bonk(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    for arg in args.iter::<String>().map(|a| a.unwrap()) {
        let uid = parse_username(&arg).ok_or("Invalid user tag")?;
        bonk_user(ctx, msg, uid, 60).await?;
    }
    Ok(())
}

#[command]
#[sub_commands(set)]
#[description("Set the server name to the default.")]
#[only_in(guilds)]
async fn defaultname(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    set_server_name(
        ctx,
        msg.guild(ctx).unwrap(),
        Some(msg),
        &db.get_guild_default_name(msg.guild_id.unwrap().0).await?,
    )
    .await
}

#[command]
#[num_args(1)]
#[description("Set what the default server name is.")]
#[required_permissions("ADMINISTRATOR")]
#[only_in(guilds)]
async fn set(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    db.set_guild_default_name(
        msg.guild_id.unwrap().0,
        args.quoted().current().unwrap().to_owned(),
    )
    .await?;
    let _ = msg.react(ctx, '🫡').await;
    Ok(())
}

#[command]
#[sub_commands(list, add_subject, add_object)]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
#[description("Set the server name to a random one.")]
async fn randomname(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let (s, o) = db.get_guild_random_names(msg.guild_id.unwrap().0).await?;
    set_server_name(ctx, msg.guild(ctx).unwrap(), Some(msg), &random_name(s, o)).await
}

#[command]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let _ = msg
        .channel_id
        .say(
            ctx,
            db.get_guild_random_names(msg.guild_id.unwrap().0)
                .await
                .map(|(s, o)| format!("Subjects: `{}`\nObjects: `{}`", s.join(", "), o.join(", ")))
                .unwrap_or_else(|e| e.to_string()),
        )
        .await;
    Ok(())
}

#[command]
#[num_args(1)]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
async fn add_subject(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    db.add_guild_random_name_subject(
        msg.guild_id.unwrap().0,
        args.quoted().current().unwrap().to_owned(),
    )
    .await?;
    let _ = msg.channel_id.say(ctx, "Added").await;
    Ok(())
}

#[command]
#[num_args(1)]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
async fn add_object(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    db.add_guild_random_name_object(
        msg.guild_id.unwrap().0,
        args.quoted().current().unwrap().to_owned(),
    )
    .await?;
    let _ = msg.channel_id.say(ctx, "Added").await;
    Ok(())
}

pub fn random_name(subs: Vec<String>, objs: Vec<String>) -> String {
    let mut rng: StdRng = SeedableRng::from_entropy();
    let sub = if subs.is_empty() {
        ""
    } else {
        &subs[rng.gen_range(0..subs.len())]
    };
    let s = sub.ends_with(|c: char| c.to_ascii_lowercase() == 's');
    let obj = if objs.is_empty() {
        ""
    } else {
        &objs[rng.gen_range(0..objs.len())]
    };
    format!("{sub}{} {obj} Klubb", if s { "" } else { "s" })
}
