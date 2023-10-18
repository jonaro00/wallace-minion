use std::time::Duration;

use rand::{rngs::StdRng, Rng, SeedableRng};
use serenity::{
    builder::{CreateEmbed, CreateEmbedAuthor, CreateMessage},
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, Command, CommandResult,
    },
    model::prelude::Message,
    utils::parse_user_mention,
};

use crate::{
    database::WallaceDBClient,
    discord::get_db_handler,
    services::{bonk_user, do_payment, nickname_user, set_server_name, unbonk_user},
};

#[group]
#[commands(
    bonk,
    gamba,
    hammer,
    unbonk,
    nickname,
    defaultname,
    servername,
    randomname
)]
struct Spells;

pub enum SpellPrice {
    Free,
    Cost(i64),
    AtLeast(i64),
}
pub static SHOPPABLE_SPELLS_AND_PRICES: &[(&Command, SpellPrice)] = &[
    (&GAMBA_COMMAND, SpellPrice::AtLeast(1)),
    (&UNBONK_COMMAND, SpellPrice::Cost(UNBONK_COST)),
    (&NICKNAME_COMMAND, SpellPrice::Cost(NICKNAME_COST)),
    (&DEFAULTNAME_COMMAND, SpellPrice::Free),
    (&SERVERNAME_COMMAND, SpellPrice::Cost(SERVERNAME_COST)),
];

#[command]
#[aliases(timeout)]
#[min_args(1)]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
#[description("Bonk a user.")]
async fn bonk(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    for arg in args.iter::<String>().map(|a| a.unwrap()) {
        let uid = parse_user_mention(&arg).ok_or("Invalid user tag")?.get();
        bonk_user(ctx, msg, uid, 60).await?;
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
#[min_args(1)]
#[max_args(3)]
#[description(
    "Summon mods in chat to start the GAMBA. Tag a user and they might get bonked. Or you.
    A larger size increases bonk time, but reduces chance of success.
    A higher bet increases the chance of success."
)]
#[usage("[S|M|L|XL|XXL] [amount] <user>")]
#[example("@Yxaria")]
#[example("M 5 @Yxaria")]
#[example("XXL 30 @Yxaria")]
async fn gamba(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let a = args.current().unwrap();
    let default: (f32, &str, u32) = (1.00, "S", 60);
    let stats: Option<(f32, &str, u32)> = match a.to_ascii_uppercase().as_str() {
        "S" => Some(default),
        "M" => Some((0.92, "M", 240)),
        "L" => Some((0.85, "L", 900)),
        "XL" => Some((0.75, "XL", 3600)),
        "XXL" => Some((0.54, "XXL", 4 * 3600)),
        _ => None,
    };
    let (modifier, size, duration) = if let Some((m, s, d)) = stats {
        args.advance();
        (m, s, d)
    } else {
        default
    };
    let a = args.current().ok_or("Not enough arguments")?;
    let amount: i64 = match a.parse() {
        Ok(n) => {
            args.advance();
            n
        }
        Err(_) => 1,
    };

    let a = args.current().ok_or("Not enough arguments")?;
    let target_uid = parse_user_mention(a).ok_or("Invalid user tag")?.get();
    let uid = msg.author.id.get();

    if do_payment(ctx, msg, amount).await.is_err() {
        return Ok(());
    }

    // m * 35 * a^(1/3)-11 bounded to [1, 100]
    let chance = ((modifier * 35.0 * (amount as f32).powf(1.0 / 3.0) - 11.0) as u32).clamp(1, 100);
    let _ = msg
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new().author(
                    CreateEmbedAuthor::new(format!(
                        "Size {size} bonk with a bet of {amount} -> {chance}% chance!"
                    ))
                    .icon_url("https://cdn.7tv.app/emote/6290c771e40c1f3cb6475a01/1x.gif"),
                ),
            ),
        )
        .await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let mut rng: StdRng = SeedableRng::from_entropy();
    let who = if rng.gen_ratio(chance, 100) {
        target_uid // Win
    } else {
        uid // Loss
    };
    bonk_user(ctx, msg, who, duration).await?;
    Ok(())
}

#[command]
#[num_args(1)]
#[only_in(guilds)]
#[description("Bonk a user. Requires a **Wallace Hammer**.")]
async fn hammer(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let _ = msg.channel_id.say(ctx, "You have no hammer 😠").await;
    return Ok(());
    // let uid = parse_user_mention(args.current().unwrap()).ok_or("Invalid user tag")?;
    // bonk_user(ctx, msg, uid, 60).await?;
    // Ok(())
}

pub const UNBONK_COST: i64 = 2;
#[command]
#[aliases(unhammer, untimeout)]
#[num_args(1)]
#[only_in(guilds)]
#[description("Unbonk a user.")]
#[usage("<user>")]
#[example("@Yxaria")]
async fn unbonk(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let uid = parse_user_mention(args.current().unwrap())
        .ok_or("Invalid user tag")?
        .get();
    if do_payment(ctx, msg, UNBONK_COST).await.is_err() {
        return Ok(());
    }
    unbonk_user(ctx, msg, uid).await
}

pub const NICKNAME_COST: i64 = 1;
#[command]
#[aliases(nick)]
#[min_args(2)]
#[only_in(guilds)]
#[description("Set the server nickname of the target user.")]
#[usage("<user> <name>")]
#[example("@Yxaria Nick 🐒 Name")]
async fn nickname(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let uid = parse_user_mention(args.current().unwrap())
        .ok_or("Invalid user tag")?
        .get();
    args.advance();
    let nick = args.rest();
    if nick.chars().count() > 32 {
        let _ = msg.channel_id.say(ctx, "Nickname too long").await;
        return Ok(());
    }
    if do_payment(ctx, msg, NICKNAME_COST).await.is_err() {
        return Ok(());
    }
    nickname_user(ctx, msg, uid, nick.to_owned()).await
}

#[command]
#[sub_commands(set)]
#[only_in(guilds)]
#[description("Set the server name to the default.")]
async fn defaultname(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let guild = msg.guild(&ctx.cache).unwrap().to_owned();
    let guild_id = guild.id;
    set_server_name(
        ctx,
        guild,
        Some(msg),
        &db.get_guild_default_name(guild_id.get()).await?,
    )
    .await
}

#[command]
#[num_args(1)]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
#[description("Set what the default server name is.")]
async fn set(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    db.set_guild_default_name(
        msg.guild_id.unwrap().get(),
        args.quoted().current().unwrap().to_owned(),
    )
    .await?;
    let _ = msg.react(ctx, '🫡').await;
    Ok(())
}

pub const SERVERNAME_COST: i64 = 3;
#[command]
#[num_args(1)]
#[only_in(guilds)]
#[description("Set the server name.")]
#[usage("<name>")]
#[example("Cool Chicken Club")]
async fn servername(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let name = args.rest();
    if do_payment(ctx, msg, SERVERNAME_COST).await.is_err() {
        return Ok(());
    }
    let guild = msg.guild(&ctx.cache).unwrap().to_owned();
    set_server_name(ctx, guild, Some(msg), name).await
}

#[command]
#[sub_commands(list, add_subject, add_object)]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
#[description("Set the server name to a random one.")]
async fn randomname(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let (s, o) = db
        .get_guild_random_names(msg.guild_id.unwrap().get())
        .await?;
    let guild = msg.guild(&ctx.cache).unwrap().to_owned();
    set_server_name(ctx, guild, Some(msg), &random_name(s, o)).await
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
            db.get_guild_random_names(msg.guild_id.unwrap().get())
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
async fn add_subject(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    db.add_guild_random_name_subject(msg.guild_id.unwrap().get(), args.rest().to_owned())
        .await?;
    let _ = msg.channel_id.say(ctx, "Added").await;
    Ok(())
}

#[command]
#[num_args(1)]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
async fn add_object(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    db.add_guild_random_name_object(msg.guild_id.unwrap().get(), args.rest().to_owned())
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
    let s = if sub.ends_with(|c: char| c.to_ascii_lowercase() == 's') {
        ""
    } else {
        "s"
    };
    let obj = if objs.is_empty() {
        ""
    } else {
        &objs[rng.gen_range(0..objs.len())]
    };
    format!("{sub}{s} {obj} Klubb")
}
