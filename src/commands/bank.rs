use std::{fmt::Write, time::Duration};

use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use serenity::{
    builder::{CreateEmbed, CreateEmbedAuthor, CreateMessage, EditMessage},
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    futures::StreamExt,
    model::prelude::Message,
    utils::parse_user_mention,
};
use tokio::time::sleep;

use super::spells::SHOPPABLE_SPELLS_AND_PRICES;
use crate::{
    database::WallaceDBClient,
    discord::{get_db_handler, PREFIX},
    services::do_payment,
};

#[group("Bank and Gambling")]
#[commands(account, shop, slots, roulette, give, mint, setmature)]
struct Bank;

#[command]
#[sub_commands(open, close, top)]
#[description("Show your account balance.")]
async fn account(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let uid = msg.author.id.get();
    let bal = match db.get_bank_account_balance(uid).await {
        Ok(b) => b,
        Err(e) => {
            let _ = msg.channel_id.say(ctx, e.to_string()).await;
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
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .author(CreateEmbedAuthor::new(format!("Balance for {uname}:")).icon_url(upic))
                    .title(format!("\\>> {bal} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻 <<"))
                    .thumbnail("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/2x.gif"),
            ),
        )
        .await;
    Ok(())
}

#[command]
#[description("Open a bank account.")]
async fn open(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let uid = msg.author.id.get();
    if let Err(e) = db.create_bank_account(uid).await {
        let _ = msg.channel_id.say(ctx, e.to_string()).await;
        return Ok(());
    }
    let _ = msg.channel_id.say(ctx, "Account opened").await;
    Ok(())
}

#[command]
#[description("Close your account.")]
async fn close(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let uid = msg.author.id.get();
    if let Err(e) = db.delete_bank_account(uid).await {
        let _ = msg.channel_id.say(ctx, e.to_string()).await;
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
        if let Ok(b) = db.get_bank_account_balance(m.user.id.get()).await {
            v.push((m, b))
        }
    }
    v.sort_by_key(|t| t.1);
    let s: String =
        v.into_iter()
            .rev()
            .take(10)
            .enumerate()
            .fold(String::new(), |mut s, (i, (m, b))| {
                writeln!(
                    &mut s,
                    "`{:>2}. {:<20} {:>4}`<:Kapsyl:1079763140272734218>",
                    i + 1,
                    m.nick.unwrap_or(m.user.name),
                    b,
                )
                .unwrap();
                s
            });
    msg.channel_id
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .author(
                        CreateEmbedAuthor::new("Top 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻 holders")
                            .icon_url("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/1x.gif"),
                    )
                    .field("", s, true),
            ),
        )
        .await?;
    Ok(())
}

#[command]
#[description("Show available buffs, items, and spells to purchase.")]
async fn shop(ctx: &Context, msg: &Message) -> CommandResult {
    let _ = msg
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .title("\\>> 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻 SHOP <<")
                    .thumbnail("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/2x.gif")
                    .colour((56, 157, 88))
                    .field("Buffs", "", false)
                    .field("Items", "", false)
                    .field(
                        "Spells",
                        SHOPPABLE_SPELLS_AND_PRICES
                            .iter()
                            .map(|(c, p)| {
                                format!(
                                    "**{}** `{}{}` {}",
                                    p,
                                    PREFIX,
                                    c.options.names[0],
                                    c.options.desc.unwrap_or_default()
                                )
                            })
                            .collect::<Vec<String>>()
                            .as_slice()
                            .join("\n"),
                        false,
                    ),
            ),
        )
        .await;
    Ok(())
}

#[derive(PartialEq)]
enum RouletteColor {
    Black,
    Red,
    Green,
}
impl RouletteColor {
    fn emoji(&self) -> char {
        match self {
            RouletteColor::Black => '⬛',
            RouletteColor::Red => '🟥',
            RouletteColor::Green => '🟩',
        }
    }
}
const ROULETTE_WHEEL_ITEMS: i8 = 17;
const ROULETTE_WHEEL: [RouletteColor; ROULETTE_WHEEL_ITEMS as usize] = [
    RouletteColor::Green,
    RouletteColor::Black,
    RouletteColor::Red,
    RouletteColor::Black,
    RouletteColor::Red,
    RouletteColor::Black,
    RouletteColor::Red,
    RouletteColor::Black,
    RouletteColor::Red,
    RouletteColor::Black,
    RouletteColor::Red,
    RouletteColor::Black,
    RouletteColor::Red,
    RouletteColor::Black,
    RouletteColor::Red,
    RouletteColor::Black,
    RouletteColor::Red,
];
fn print_roulette(i: i8, locked: bool) -> String {
    let w = "▫";
    format!(
        "{}                                        {}\n{}{}🔽{}{}\n{}{}{}{}{}",
        ROULETTE_WHEEL[((i - 4).rem_euclid(ROULETTE_WHEEL_ITEMS)) as usize].emoji(),
        ROULETTE_WHEEL[((i + 4).rem_euclid(ROULETTE_WHEEL_ITEMS)) as usize].emoji(),
        ROULETTE_WHEEL[((i - 3).rem_euclid(ROULETTE_WHEEL_ITEMS)) as usize].emoji(),
        if locked { w } else { "             " },
        if locked { w } else { "             " },
        ROULETTE_WHEEL[((i + 3).rem_euclid(ROULETTE_WHEEL_ITEMS)) as usize].emoji(),
        ROULETTE_WHEEL[((i - 2).rem_euclid(ROULETTE_WHEEL_ITEMS)) as usize].emoji(),
        ROULETTE_WHEEL[((i - 1).rem_euclid(ROULETTE_WHEEL_ITEMS)) as usize].emoji(),
        ROULETTE_WHEEL[i as usize].emoji(),
        ROULETTE_WHEEL[((i + 1).rem_euclid(ROULETTE_WHEEL_ITEMS)) as usize].emoji(),
        ROULETTE_WHEEL[((i + 2).rem_euclid(ROULETTE_WHEEL_ITEMS)) as usize].emoji(),
    )
}

#[command]
#[bucket = "slots"]
#[num_args(2)]
#[aliases(roll)]
#[description(
    "Scuffed roulette! Win up to 16x your bet! User must be marked mature to get access."
)]
#[usage("<amount> <B|R|G>")]
#[example("1 b")]
async fn roulette(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let amount: i64 = args
        .current()
        .unwrap()
        .parse()
        .map_err(|_| "Invalid amount")?;
    args.advance();
    let bet = match args.current().unwrap().to_ascii_uppercase().as_str() {
        "B" => RouletteColor::Black,
        "R" => RouletteColor::Red,
        "G" => RouletteColor::Green,
        _ => {
            let _ = msg.channel_id.say(ctx, "Choose B, R, or G.").await;
            return Ok(());
        }
    };
    let uid = msg.author.id.get();
    let db = get_db_handler(ctx).await;
    if !db.get_user_mature(uid).await? {
        let _ = msg
            .channel_id
            .say(ctx, "User must be marked as mature ☝🤓")
            .await;
        return Ok(());
    }
    if do_payment(ctx, msg, amount).await.is_err() {
        return Ok(());
    }
    let mut rng: StdRng = SeedableRng::from_entropy();
    let mut counter: i8 = rng.gen_range(0..ROULETTE_WHEEL_ITEMS);
    let mut m = msg
        .channel_id
        .say(ctx, print_roulette(counter, false))
        .await?;
    for _ in 0..rng.gen_range(7..12) {
        sleep(DELAY_BETWEEN_EDITS).await;
        counter = (counter + 1).rem_euclid(ROULETTE_WHEEL_ITEMS);
        let _ = m
            .edit(
                ctx,
                EditMessage::new().content(print_roulette(counter, false)),
            )
            .await;
    }

    sleep(DELAY_BETWEEN_EDITS).await;
    let _ = m
        .edit(
            ctx,
            EditMessage::new().content(print_roulette(counter, true)),
        )
        .await;

    let (amount, result) = if bet == ROULETTE_WHEEL[counter as usize] {
        match ROULETTE_WHEEL[counter as usize] {
            RouletteColor::Black => (2 * amount, "Black"),
            RouletteColor::Red => (2 * amount, "Red"),
            RouletteColor::Green => ((ROULETTE_WHEEL_ITEMS - 1) as i64 * amount, "Green"),
        }
    } else {
        (0, "")
    };
    if amount == 0 {
        return Ok(());
    }
    if let Err(e) = db.add_bank_account_balance(uid, amount).await {
        let _ = msg.channel_id.say(ctx, e.to_string()).await;
        return Ok(());
    }
    let _ = msg
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .author(
                        CreateEmbedAuthor::new("Win!")
                            .icon_url("https://cdn.7tv.app/emote/628d8b64ed0a40a5ec5f4810/1x.gif"),
                    )
                    .title(format!("{result}! Gained {amount} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻!")),
            ),
        )
        .await;
    Ok(())
}

#[derive(Clone, PartialEq)]
struct SlotItem {
    item: SlotItemType,
    material: SlotItemMaterial,
    color: SlotItemColor,
}
#[derive(Clone, Debug, PartialEq)]
enum SlotItemType {
    Crown,
    Ring,
    Hammer,
    Crab,
    Cherry,
    Grapes,
    Blueberries,
    Pear,
    Apple,
}
impl SlotItemType {
    fn emoji(&self) -> char {
        match self {
            Self::Crown => '👑',
            Self::Ring => '💍',
            Self::Hammer => '🔨',
            Self::Crab => '🦀',
            Self::Cherry => '🍒',
            Self::Grapes => '🍇',
            Self::Blueberries => '🫐',
            Self::Pear => '🍐',
            Self::Apple => '🍏',
        }
    }
}
#[derive(Clone, PartialEq)]
enum SlotItemMaterial {
    Metal,
    Fruit,
    None,
}
#[derive(Clone, PartialEq)]
enum SlotItemColor {
    Red,
    Purple,
    Green,
    None,
}
const SLOT_WHEEL_ITEMS: i8 = 13;
const SLOT_WHEEL: [SlotItem; SLOT_WHEEL_ITEMS as usize] = [
    SlotItem {
        item: SlotItemType::Crown,
        material: SlotItemMaterial::Metal,
        color: SlotItemColor::None,
    },
    SlotItem {
        item: SlotItemType::Ring,
        material: SlotItemMaterial::Metal,
        color: SlotItemColor::None,
    },
    SlotItem {
        item: SlotItemType::Hammer,
        material: SlotItemMaterial::Metal,
        color: SlotItemColor::None,
    },
    SlotItem {
        item: SlotItemType::Crab,
        material: SlotItemMaterial::None,
        color: SlotItemColor::Red,
    },
    SlotItem {
        item: SlotItemType::Cherry,
        material: SlotItemMaterial::Fruit,
        color: SlotItemColor::Red,
    },
    SlotItem {
        item: SlotItemType::Grapes,
        material: SlotItemMaterial::Fruit,
        color: SlotItemColor::Purple,
    },
    SlotItem {
        item: SlotItemType::Grapes,
        material: SlotItemMaterial::Fruit,
        color: SlotItemColor::Purple,
    },
    SlotItem {
        item: SlotItemType::Blueberries,
        material: SlotItemMaterial::Fruit,
        color: SlotItemColor::Purple,
    },
    SlotItem {
        item: SlotItemType::Blueberries,
        material: SlotItemMaterial::Fruit,
        color: SlotItemColor::Purple,
    },
    SlotItem {
        item: SlotItemType::Pear,
        material: SlotItemMaterial::Fruit,
        color: SlotItemColor::Green,
    },
    SlotItem {
        item: SlotItemType::Pear,
        material: SlotItemMaterial::Fruit,
        color: SlotItemColor::Green,
    },
    SlotItem {
        item: SlotItemType::Apple,
        material: SlotItemMaterial::Fruit,
        color: SlotItemColor::Green,
    },
    SlotItem {
        item: SlotItemType::Apple,
        material: SlotItemMaterial::Fruit,
        color: SlotItemColor::Green,
    },
];
fn random_wheel(rng: &mut StdRng) -> Vec<SlotItem> {
    let mut v: Vec<SlotItem> = SLOT_WHEEL.iter().map(Clone::clone).collect();
    v.shuffle(rng);
    v
}
fn calculate_payout_result(i1: &SlotItem, i2: &SlotItem, i3: &SlotItem) -> (i64, String) {
    let (amount, s) = match (i1, i2, i3) {
        (i, j, k) if i == j && j == k => match i.item {
            SlotItemType::Crown => (333, "Three Crowns"),
            SlotItemType::Ring => (66, "Three Rings"),
            SlotItemType::Hammer => (55, "Three Hammers"),
            SlotItemType::Crab => (44, "Three Crabs"),
            SlotItemType::Cherry => (33, "Three Cherrys"),
            SlotItemType::Grapes => (14, "Three Grapes"),
            SlotItemType::Blueberries => (13, "Three Blueberries"),
            SlotItemType::Pear => (12, "Three Pears"),
            SlotItemType::Apple => (11, "Three Apples"),
        },
        (i, j, k) if i.color != SlotItemColor::None && i.color == j.color && j.color == k.color => {
            match i.color {
                SlotItemColor::Red => (8, "Three Red"),
                SlotItemColor::Purple => (4, "Three Purple"),
                SlotItemColor::Green => (2, "Three Green"),
                SlotItemColor::None => panic!(),
            }
        }
        (i, j, k)
            if i.material != SlotItemMaterial::None
                && i.material == j.material
                && j.material == k.material =>
        {
            match i.material {
                SlotItemMaterial::Metal => (7, "Three Shiny Metal Objects"),
                SlotItemMaterial::Fruit => (1, "Three Fruits"),
                SlotItemMaterial::None => panic!(),
            }
        }
        _ => (0, ""),
    };
    (amount, s.to_string())
}
fn print_slots(
    vs: &(Vec<SlotItem>, Vec<SlotItem>, Vec<SlotItem>),
    is: &[i8; 3],
    locked: u8,
) -> String {
    let (v1, v2, v3) = vs;
    let (i1, i2, i3) = (is[0], is[1], is[2]);
    let w = '▫';
    format!(
        "{}{}{}\n{}{}{}\n{}{}{}",
        if locked < 1 {
            v1[((i1 - 1).rem_euclid(SLOT_WHEEL_ITEMS)) as usize]
                .item
                .emoji()
        } else {
            w
        },
        if locked < 2 {
            v2[((i2 - 1).rem_euclid(SLOT_WHEEL_ITEMS)) as usize]
                .item
                .emoji()
        } else {
            w
        },
        if locked < 3 {
            v3[((i3 - 1).rem_euclid(SLOT_WHEEL_ITEMS)) as usize]
                .item
                .emoji()
        } else {
            w
        },
        v1[i1 as usize].item.emoji(),
        v2[i2 as usize].item.emoji(),
        v3[i3 as usize].item.emoji(),
        if locked < 1 {
            v1[((i1 + 1).rem_euclid(SLOT_WHEEL_ITEMS)) as usize]
                .item
                .emoji()
        } else {
            w
        },
        if locked < 2 {
            v2[((i2 + 1).rem_euclid(SLOT_WHEEL_ITEMS)) as usize]
                .item
                .emoji()
        } else {
            w
        },
        if locked < 3 {
            v3[((i3 + 1).rem_euclid(SLOT_WHEEL_ITEMS)) as usize]
                .item
                .emoji()
        } else {
            w
        },
    )
}

const DELAY_BETWEEN_EDITS: Duration = Duration::from_millis(800);
#[command]
#[bucket = "slots"]
#[description(
    "Try your luck at the casino.
    Costs 1 𝓚𝓪𝓹𝓼𝔂𝓵, but you can win up to 333 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻!
    User must be marked mature to get access."
)]
async fn slots(ctx: &Context, msg: &Message) -> CommandResult {
    let uid = msg.author.id.get();
    let db = get_db_handler(ctx).await;
    if !db.get_user_mature(uid).await? {
        let _ = msg
            .channel_id
            .say(ctx, "User must be marked as mature ☝🤓")
            .await;
        return Ok(());
    }
    if do_payment(ctx, msg, 1).await.is_err() {
        return Ok(());
    }
    let mut rng: StdRng = SeedableRng::from_entropy();
    let wheels = (
        random_wheel(&mut rng),
        random_wheel(&mut rng),
        random_wheel(&mut rng),
    );
    let counters: &mut [i8; 3] = &mut [0, 0, 0];
    let mut m = msg
        .channel_id
        .say(ctx, print_slots(&wheels, counters, 0))
        .await?;
    for i in 0..3 {
        for _ in 0..rng.gen_range(1..8) {
            sleep(DELAY_BETWEEN_EDITS).await;
            for j in counters.iter_mut().take(3).skip(i) {
                *j = (*j - 1).rem_euclid(SLOT_WHEEL_ITEMS);
            }
            let _ = m
                .edit(
                    ctx,
                    EditMessage::new().content(print_slots(&wheels, counters, i as u8)),
                )
                .await;
        }
    }
    sleep(DELAY_BETWEEN_EDITS).await;
    let _ = m
        .edit(
            ctx,
            EditMessage::new().content(print_slots(&wheels, counters, 3)),
        )
        .await;

    let (amount, result) = calculate_payout_result(
        &wheels.0[counters[0] as usize],
        &wheels.1[counters[1] as usize],
        &wheels.2[counters[2] as usize],
    );
    if amount == 0 {
        return Ok(());
    }
    if let Err(e) = db.add_bank_account_balance(uid, amount).await {
        let _ = msg.channel_id.say(ctx, e.to_string()).await;
        return Ok(());
    }
    let _ = msg
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new()
                    .author(
                        CreateEmbedAuthor::new("Win!")
                            .icon_url("https://cdn.7tv.app/emote/628d8b64ed0a40a5ec5f4810/1x.gif"),
                    )
                    .title(format!("🟩 {result}! Gained {amount} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻!")),
            ),
        )
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
    let target_uid = parse_user_mention(a).ok_or("Invalid user tag")?.get();
    let uid = msg.author.id.get();
    if let Err(e) = db
        .transfer_bank_account_balance(uid, target_uid, amount)
        .await
    {
        let _ = msg.channel_id.say(ctx, e.to_string()).await;
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
        .send_message(
            ctx,
            CreateMessage::new().add_embed(
                CreateEmbed::new().author(
                    CreateEmbedAuthor::new(format!("Gave {amount} 𝓚𝓪𝓹𝓼𝔂𝓵𝓮𝓻 to {tn}."))
                        .icon_url("https://cdn.7tv.app/emote/60edf43ba60faa2a91cfb082/1x.gif"),
                ),
            ),
        )
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
    let uid = msg.author.id.get();
    if let Err(e) = db.add_bank_account_balance(uid, amount).await {
        let _ = msg.channel_id.say(ctx, e.to_string()).await;
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
async fn setmature(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let a = args.current().unwrap();
    let target_uid = parse_user_mention(a).ok_or("Invalid user tag")?.get();
    args.advance();
    let a = args.current().unwrap();
    let mature: bool = a.parse().map_err(|_| "Invalid bool")?;
    if let Err(e) = db.set_user_mature(target_uid, mature).await {
        let _ = msg.channel_id.say(ctx, e.to_string()).await;
        return Ok(());
    }
    let _ = msg.react(ctx, '🫡').await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slots() {
        let mut won = 0;
        let mut loss = 0;
        let mut neus = 0;
        let mut wins = 0;
        let mut rng: StdRng = SeedableRng::from_entropy();
        let spent = 1000000;
        for _ in 0..spent {
            let (amount, _) = calculate_payout_result(
                &SLOT_WHEEL[rng.gen_range(0..SLOT_WHEEL.len())],
                &SLOT_WHEEL[rng.gen_range(0..SLOT_WHEEL.len())],
                &SLOT_WHEEL[rng.gen_range(0..SLOT_WHEEL.len())],
            );
            if amount == 0 {
                loss += 1;
            } else if amount == 1 {
                neus += 1;
            } else {
                wins += 1;
            }
            won += amount;
        }
        println!("Loss: {loss}, Neus: {neus}, Wins: {wins}, Spent: {spent}, Won: {won}");
    }
}
