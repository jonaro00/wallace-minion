use std::time::Duration;

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

use crate::{discord::get_db_handler, services::bonk_user};

#[group]
#[commands(account, gamba, coinflip, give, mint)]
struct Bank;

#[command]
#[sub_commands(open, close)]
#[description("Show your account balance.")]
async fn account(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let uid = msg.author.id.0;
    let bal = match db.get_bank_account_balance(uid).await {
        Ok(b) => b,
        Err(e) => {
            let _ = msg.channel_id.say(ctx, e).await;
            return Ok(());
        }
    };
    let uname = &msg.author.name;
    let upic = &msg
        .author
        .avatar_url()
        .unwrap_or("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/2x.gif".into());
    let _ = msg
        .channel_id
        .send_message(ctx, |m| {
            m.add_embed(|e| {
                e.author(|a| a.name(format!("Balance for {uname}:")).icon_url(upic))
                    .title(format!("\\>> {bal} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻 <<"))
                    .thumbnail("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/2x.gif")
            })
        })
        .await;
    Ok(())
}

#[command]
#[description("Open a bank account.")]
async fn open(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let uid = msg.author.id.0;
    if let Err(e) = db.create_bank_account(uid).await {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }
    let _ = msg.channel_id.say(ctx, "Account opened").await;
    Ok(())
}

#[command]
#[description("Close your account.")]
async fn close(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let uid = msg.author.id.0;
    if let Err(e) = db.delete_bank_account(uid).await {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }
    let _ = msg.channel_id.say(ctx, "Account closed").await;
    Ok(())
}

#[command]
#[only_in(guilds)]
#[min_args(1)]
#[max_args(3)]
#[description(
    "Summon mods in chat to start the GAMBA. Tag a user and they might get bonked. Or you."
)]
#[usage("[S|M|L|XL|XXL] [amount] <user>")]
#[example("@Yxaria")]
#[example("M 5 @Yxaria")]
async fn gamba(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let a = args.current().unwrap();
    let stats: Option<(f32, &str, u32)> = match a.to_ascii_uppercase().as_str() {
        "S" => Some((1.00, "S", 60)),
        "M" => Some((0.92, "M", 180)),
        "L" => Some((0.85, "L", 600)),
        "XL" => Some((0.75, "XL", 1800)),
        "XXL" => Some((0.54, "XXL", 3600)),
        _ => None,
    };
    let (modifier, size, duration) = if let Some((m, s, d)) = stats {
        args.advance();
        (m, s, d)
    } else {
        (1.00, "S", 60)
    };
    let a = args.current().ok_or("Not enough arguments")?;
    let amount: i64 = match a.parse() {
        Ok(n) => {
            args.advance();
            1.max(n)
        }
        Err(_) => 1,
    };

    let a = args.current().ok_or("Not enough arguments")?;
    let target_uid = parse_username(a).ok_or("Invalid user tag")?;

    let uid = msg.author.id.0;
    if let Err(e) = db.modify_bank_account_balance(uid, -amount).await {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }

    // m * 35 * a^(1/3)-11 bounded to [1, 100]
    let chance = 100.min(1.max((modifier * 35.0 * (amount as f32).powf(1.0 / 3.0) - 11.0) as u32));
    let _ = msg
        .channel_id
        .send_message(ctx, |m| {
            m.add_embed(|e| {
                e.author(|a| {
                    a.name(format!(
                        "Size {size} bonk with a bet of {amount} -> {chance}% chance!"
                    ))
                    .icon_url("https://cdn.7tv.app/emote/6290c771e40c1f3cb6475a01/1x.gif")
                })
            })
        })
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
#[description("Double or nothing!")]
#[usage("<amount>")]
#[example("1")]
async fn coinflip(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let amount: i64 = 1.max(
        args.current()
            .unwrap()
            .parse()
            .map_err(|_| "Invalid amount")?,
    );
    let uid = msg.author.id.0;
    let db = get_db_handler(ctx).await;
    let mut rng: StdRng = SeedableRng::from_entropy();
    let (diff, reply) = if rng.gen_ratio(1, 2) {
        // Win
        (amount, format!("🟩 Gained {amount} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻!"))
    } else {
        // Loss
        (-amount, format!("🟥 Lost {amount} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻!"))
    };
    if let Err(e) = db.modify_bank_account_balance(uid, diff).await {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }
    let _ = msg
        .channel_id
        .send_message(ctx, |m| {
            m.add_embed(|e| {
                e.author(|a| {
                    a.name(format!("Coin flip. 50% chance!"))
                        .icon_url("https://cdn.7tv.app/emote/61e63db277175547b425ce27/1x.gif")
                })
                .title(reply)
            })
        })
        .await;
    Ok(())
}

#[command]
#[only_in(guilds)]
#[num_args(2)]
#[description("Give 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻 to someone.")]
#[usage("<amount> <user>")]
#[example("5 @Yxaria")]
async fn give(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let a = args.current().unwrap();
    let amount: i64 = 1.max(a.parse().map_err(|_| "Invalid amount")?);
    args.advance();
    let a = args.current().unwrap();
    let target_uid = parse_username(a).ok_or("Invalid user tag")?;
    let uid = msg.author.id.0;
    if let Err(e) = db
        .transfer_bank_account_balance(uid, target_uid, amount)
        .await
    {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }
    let tn = msg.guild_id.unwrap().member(ctx, target_uid).await.map(|m| m.nick.unwrap_or("?".into())).unwrap_or_else(|_| "?".into());
    let _ = msg
        .channel_id
        .send_message(ctx, |m| {
            m.add_embed(|e| {
                e.author(|a| {
                    a.name(format!("Gave {amount} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻 to {tn}."))
                        .icon_url("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/1x.gif")
                })
            })
        })
        .await;
    Ok(())
}

#[command]
#[owners_only]
#[num_args(1)]
#[description("Make 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻. 🤨")]
#[usage("<amount>")]
#[example("9999")]
async fn mint(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let a = args.current().unwrap();
    let amount: i64 = 1.max(a.parse().map_err(|_| "Invalid amount")?);
    let uid = msg.author.id.0;
    if let Err(e) = db.modify_bank_account_balance(uid, amount).await {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }
    let _ = msg.react(ctx, '🫡').await;
    Ok(())
}
