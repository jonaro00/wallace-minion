use rand::{rngs::StdRng, Rng, SeedableRng};
use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    futures::StreamExt,
    model::prelude::Message,
    utils::parse_username,
};

use super::spells::{SpellPrice, SHOPPABLE_SPELLS_AND_PRICES};
use crate::{
    database::WallaceDBClient,
    discord::{get_db_handler, PREFIX},
};

#[group]
#[commands(account, shop, coinflip, give, mint, set_mature)]
struct Bank;

#[command]
#[sub_commands(open, close, top)]
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
        .unwrap_or_else(|| "https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/2x.gif".into());
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
#[description("See the top 𝓚𝓪𝓹𝓼𝔂𝓵 holders in this guild.")]
async fn top(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let mut mem = msg.guild_id.unwrap().members_iter(ctx).boxed();
    let mut v = vec![];
    while let Some(Ok(m)) = mem.next().await {
        if let Ok(b) = db.get_bank_account_balance(m.user.id.0).await {
            v.push((m, b))
        }
    }
    v.sort_by_key(|t| t.1);
    let s: String = v
        .into_iter()
        .rev()
        .take(5)
        .enumerate()
        .map(|(i, (m, b))| {
            format!(
                "`{}. {:<19} {:>3}`\n",
                i + 1,
                m.nick.unwrap_or(m.user.name),
                b
            )
        })
        .collect();
    msg.channel_id
        .send_message(ctx, |m| {
            m.add_embed(|e| {
                e.author(|a| {
                    a.name("Top 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻 holders")
                        .icon_url("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/1x.gif")
                })
                .title(s)
            })
        })
        .await?;
    Ok(())
}

#[command]
#[description("Show available buffs, items, and spells to purchase.")]
async fn shop(ctx: &Context, msg: &Message) -> CommandResult {
    let _ = msg
        .channel_id
        .send_message(ctx, |m| {
            m.add_embed(|e| {
                e.title("\\>> 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻 SHOP <<")
                    .thumbnail("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/2x.gif")
                    .field("Buffs", "", false)
                    .field("Items", "", false)
                    .field(
                        "Spells",
                        SHOPPABLE_SPELLS_AND_PRICES
                            .iter()
                            .map(|(c, p)| {
                                format!(
                                    "**{}{}** `{}{}` {}",
                                    match p {
                                        SpellPrice::Free => "Free".into(),
                                        SpellPrice::Cost(q) => q.to_string(),
                                        SpellPrice::AtLeast(q) => format!("{q}+"),
                                    },
                                    if let SpellPrice::Free = p {
                                        ""
                                    } else {
                                        " 𝓚"
                                    },
                                    PREFIX,
                                    c.options.names[0],
                                    c.options.desc.unwrap_or_default()
                                )
                            })
                            .collect::<Vec<String>>()
                            .as_slice()
                            .join("\n"),
                        false,
                    )
            })
        })
        .await;
    Ok(())
}

#[command]
#[num_args(1)]
#[aliases(cflip, kflip)]
#[description("Double or nothing! User must be marked mature to get access.")]
#[usage("<amount>")]
#[example("1")]
async fn coinflip(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let amount: i64 = args
        .current()
        .unwrap()
        .parse()
        .map_err(|_| "Invalid amount")?;
    let uid = msg.author.id.0;
    let db = get_db_handler(ctx).await;
    if !db.get_user_mature(uid).await? {
        let _ = msg
            .channel_id
            .say(ctx, "User must be marked as mature ☝🤓")
            .await;
        return Ok(());
    }
    let mut rng: StdRng = SeedableRng::from_entropy();
    if let Err(e) = db.has_bank_account_balance(uid, amount).await {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }
    let (res, reply) = if rng.gen_ratio(1, 2) {
        // Win
        (
            db.add_bank_account_balance(uid, amount).await,
            format!("🟩 Gained {amount} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻!"),
        )
    } else {
        // Loss
        (
            db.subtract_bank_account_balance(uid, amount).await,
            format!("🟥 Lost {amount} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻!"),
        )
    };
    if let Err(e) = res {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }
    let _ = msg
        .channel_id
        .send_message(ctx, |m| {
            m.add_embed(|e| {
                e.author(|a| {
                    a.name("Coin flip. 50% chance!")
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
    let amount: i64 = a.parse().map_err(|_| "Invalid amount")?;
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
    let tn = msg
        .guild_id
        .unwrap()
        .member(ctx, target_uid)
        .await
        .map(|m| m.nick.unwrap_or(m.user.name))
        .unwrap_or_else(|_| "?".into());
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
    let amount: i64 = a.parse().map_err(|_| "Invalid amount")?;
    let uid = msg.author.id.0;
    if let Err(e) = db.add_bank_account_balance(uid, amount).await {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }
    let _ = msg.react(ctx, '🫡').await;
    Ok(())
}

#[command]
#[owners_only]
#[num_args(2)]
#[usage("<user> true|false")]
#[example("@Yxaria true")]
async fn set_mature(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let a = args.current().unwrap();
    let target_uid = parse_username(a).ok_or("Invalid user tag")?;
    args.advance();
    let a = args.current().unwrap();
    let mature: bool = a.parse().map_err(|_| "Invalid bool")?;
    if let Err(e) = db.set_user_mature(target_uid, mature).await {
        let _ = msg.channel_id.say(ctx, e).await;
        return Ok(());
    }
    let _ = msg.react(ctx, '🫡').await;
    Ok(())
}
