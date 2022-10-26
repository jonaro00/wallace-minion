mod set_store;

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serenity::async_trait;
use serenity::framework::standard::macros::{command, group, hook};
use serenity::framework::standard::{CommandResult, StandardFramework, Args};
use serenity::http::Http;
use serenity::model::prelude::{Activity, Guild, GuildId, Message, Ready};
use serenity::prelude::*;
use time::{OffsetDateTime, Weekday};
use tokio::time::sleep;

use set_store::SetStore;

const GUILD_FILE: &str = "active_guilds.txt";

const GUILD_DEFAULT_NAME: &str = "Tisdags Gaming Klubb";

const GUILD_NAME_SUBJECTS: [&str; 35] = [
    "Tisdag",
    "Johan",
    "Matteus",
    "Daniel",
    "Gabriel",
    "Mattias",
    "Olle",
    "Wilmer",
    "Vincent",
    "NallePuh",
    "Ior",
    "Bompadraken",
    "GrodanBoll",
    "Anki",
    "Pettson",
    "FågelTurken",
    "Pingu",
    "Muminpappa",
    "LillaMy",
    "Lipton",
    "Gordon",
    "Wallace",
    "Gromit",
    "KorvIngvar",
    "Knugen",
    "EggMan",
    "Trump",
    "Svergie",
    "Kinas",
    "GustavVasa",
    "Trollface",
    "MackaPacka",
    "Svampbob",
    "Perry",
    "DrDoofenshmirtz",
];
const GUILD_NAME_OBJECTS: [&str; 34] = [
    "Gaming",
    "Minecraft",
    "Fortnite",
    "LoL",
    "Gartic",
    "AmongUs",
    "Terraria",
    "Pokémon",
    "Magic",
    "Schack",
    "Ost",
    "Korv",
    "Blodpudding",
    "Potatisbulle",
    "Whiskey",
    "Chips",
    "Ägg",
    "BingChilling",
    "Nyponsoppa",
    "Grönsaks",
    "👺",
    "Anime",
    "Kpop",
    "Matematik",
    "Plugg",
    "Kubb",
    "Nörd",
    "Hatt",
    "Pingvin",
    "Välfärds",
    "Ekonomi",
    "Ondskefulla",
    "Hemliga",
    "Trädgårds",
];

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let token =
        env::var("DISCORD_TOKEN").expect("Discord token missing! (env variable `DISCORD_TOKEN`)");

    let http = Http::new(&token);
    let (owners, _bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else {
                owners.insert(info.owner.id);
            }
            match http.get_current_user().await {
                Ok(bot) => (owners, bot.id),
                Err(why) => panic!("Could not access the bot id: {:?}", why),
            }
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!").owners(owners))
        .unrecognised_command(unknown_command)
        .group(&GENERAL_GROUP)
        .group(&COOLTEXT_GROUP);
    let mut client = Client::builder(
        token,
        GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT,
    )
    .event_handler(Handler)
    .framework(framework)
    .await
    .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {why:?}");
    }
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        tokio::spawn(schedule_loop(ctx, guilds)).await.unwrap();
    }

    async fn ready(&self, ctx: Context, data: Ready) {
        println!("{} is connected!", data.user.name);
        let _ = ctx.set_activity(Activity::watching("you 🔨🙂")).await;
    }
}

#[hook]
async fn unknown_command(ctx: &Context, msg: &Message, unknown_command_name: &str) {
    let _ = msg
        .channel_id
        .say(
            ctx,
            format!("Me not understand '{unknown_command_name}' 🤔"),
        )
        .await;
}

async fn admin_command_check(ctx: &Context, msg: &Message) -> bool {
    let guild = msg.guild(ctx).unwrap();
    match guild.member_permissions(ctx, &msg.author.id).await {
        Ok(perms) => {
            if perms.administrator() {
                return true;
            }
        }
        Err(e) => println!(
            "Error getting permissions for user {}: {}",
            &msg.author.id, e
        ),
    };
    let _ = msg.channel_id.say(&ctx.http, "You can't do that 😛").await;
    false
}

#[group]
#[commands(ping, defaultname, randomname, randomnameon, randomnameoff)]
struct General;

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    if msg.author.id.to_string() == "224233166024474635" {
        let _ = msg.react(ctx, '👑').await;
    }
    let _ = msg.react(ctx, '👍').await;
    msg.channel_id.say(ctx, "Pong!").await?;
    Ok(())
}

#[command]
#[only_in(guilds)]
// #[required_permissions("ADMINISTRATOR")]
async fn defaultname(ctx: &Context, msg: &Message) -> CommandResult {
    if !admin_command_check(ctx, msg).await {
        return Ok(());
    }
    set_server_name(ctx, msg.guild(ctx).unwrap(), Some(msg), GUILD_DEFAULT_NAME).await
}

#[command]
#[only_in(guilds)]
// #[required_permissions("ADMINISTRATOR")]
async fn randomname(ctx: &Context, msg: &Message) -> CommandResult {
    if !admin_command_check(ctx, msg).await {
        return Ok(());
    }
    set_server_name(ctx, msg.guild(ctx).unwrap(), Some(msg), &random_name()).await
}

fn random_name() -> String {
    let mut rng: StdRng = SeedableRng::from_entropy();
    let sub = GUILD_NAME_SUBJECTS[rng.gen_range(0..GUILD_NAME_SUBJECTS.len())];
    let s = sub.ends_with(|c| c == 's' || c == 'S');
    let obj = GUILD_NAME_OBJECTS[rng.gen_range(0..GUILD_NAME_OBJECTS.len())];
    format!("{sub}{} {obj} Klubb", if s { "" } else { "s" })
}

async fn set_server_name(
    ctx: &Context,
    mut guild: Guild,
    reply_to: Option<&Message>,
    name: &str,
) -> CommandResult {
    guild.edit(ctx, |g| g.name(name)).await?;
    if let Some(msg) = reply_to {
        msg.channel_id
            .say(ctx, format!("Set server name to '{}'", name))
            .await?;
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
// #[required_permissions("ADMINISTRATOR")]
async fn randomnameon(ctx: &Context, msg: &Message) -> CommandResult {
    if !admin_command_check(ctx, msg).await {
        return Ok(());
    }
    let mut s = SetStore::new(PathBuf::from(GUILD_FILE))?;
    if let Err(_) = s.insert(msg.guild(ctx).unwrap().id.0) {
        msg.channel_id.say(ctx, "Epic fail in the system").await?;
        return Ok(());
    };
    msg.channel_id
        .say(ctx, "Added server to receive random names every night")
        .await?;
    Ok(())
}

#[command]
#[only_in(guilds)]
// #[required_permissions("ADMINISTRATOR")]
async fn randomnameoff(ctx: &Context, msg: &Message) -> CommandResult {
    if !admin_command_check(ctx, msg).await {
        return Ok(());
    }
    let mut s = SetStore::new(PathBuf::from(GUILD_FILE))?;
    if let Err(_) = s.remove(msg.guild(ctx).unwrap().id.0) {
        msg.channel_id.say(ctx, "Epic fail in the system").await?;
        return Ok(());
    };
    msg.channel_id
        .say(ctx, "Removed server to receive random names every night")
        .await?;
    Ok(())
}

#[group]
#[commands(cooltext)]
struct CoolText;

#[command]
#[aliases(ct)]
#[sub_commands(boldfraktur, bold, bolditalic, boldscript, monospace)]
async fn cooltext(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    msg.reply(ctx, to_cool_text(args.rest(), CoolTextTypes::BoldFraktur)).await?; // default
    Ok(())
}
#[command]
#[aliases(bf)]
async fn boldfraktur(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    msg.reply(ctx, to_cool_text(args.rest(), CoolTextTypes::BoldFraktur)).await?;
    Ok(())
}
#[command]
#[aliases(b)]
async fn bold(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    msg.reply(ctx, to_cool_text(args.rest(), CoolTextTypes::Bold)).await?;
    Ok(())
}
#[command]
#[aliases(bi)]
async fn bolditalic(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    msg.reply(ctx, to_cool_text(args.rest(), CoolTextTypes::BoldItalic)).await?;
    Ok(())
}
#[command]
#[aliases(bs)]
async fn boldscript(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    msg.reply(ctx, to_cool_text(args.rest(), CoolTextTypes::BoldScript)).await?;
    Ok(())
}
#[command]
#[aliases(m)]
async fn monospace(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    msg.reply(ctx, to_cool_text(args.rest(), CoolTextTypes::Monospace)).await?;
    Ok(())
}
enum CoolTextTypes {
    BoldFraktur,
    Bold,
    BoldItalic,
    BoldScript,
    Monospace,
}
fn cool_text_bases(variant: CoolTextTypes) -> Bases {
    match variant {
        CoolTextTypes::BoldFraktur => (Some(0x1D56C), Some(0x1D586), None),
        CoolTextTypes::Bold => (Some(0x1D400), Some(0x1D41A), Some(0x1D7CE)),
        CoolTextTypes::BoldItalic => (Some(0x1D468), Some(0x1D482), None),
        CoolTextTypes::BoldScript => (Some(0x1D4D0), Some(0x1D4EA), None),
        CoolTextTypes::Monospace => (Some(0x1D670), Some(0x1D68A), Some(0x1D7F6)),
    }
}
type Bases = (Option<u32>, Option<u32>, Option<u32>); // upper, lower, numeric
const ASCII_BASES: Bases = (Some(0x41), Some(0x61), Some(0x30));
fn to_cool_text(text: &str, variant: CoolTextTypes) -> String {
    let offsets = cool_text_bases(variant);
    let mut s = String::new();
    for c in text.chars() {
        if c.is_ascii_uppercase() && offsets.0.is_some() {
            s.push(char::from_u32((c as u32) - ASCII_BASES.0.unwrap() + offsets.0.unwrap()).unwrap_or_else(|| c));
        }
        else if c.is_ascii_lowercase() && offsets.1.is_some() {
            s.push(char::from_u32((c as u32) - ASCII_BASES.1.unwrap() + offsets.1.unwrap()).unwrap_or_else(|| c));
        }
        else if c.is_ascii_digit() && offsets.2.is_some() {
            s.push(char::from_u32((c as u32) - ASCII_BASES.2.unwrap() + offsets.2.unwrap()).unwrap_or_else(|| c));
        }
        else {
            s.push(c);
        }
    }
    s
}

fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

async fn schedule_loop(ctx: Context, guilds: Vec<GuildId>) {
    let mut prev_time = get_time();
    loop {
        let time = get_time();
        let day = time.weekday();
        let hour = time.hour();
        let minute = time.minute();
        if day != prev_time.weekday() {
            nightly_name_update(&ctx, &guilds, &day).await;
        }
        if hour != prev_time.hour() {}
        if minute != prev_time.minute() {}
        prev_time = time;
        sleep(Duration::from_secs(60)).await;
    }
}

async fn nightly_name_update(ctx: &Context, guilds: &Vec<GuildId>, day: &Weekday) {
    println!("It's a new day ({day}) :D");
    let members = match SetStore::new(PathBuf::from(GUILD_FILE)) {
        Ok(m) => m,
        Err(_) => {
            println!("Failed to load name change members :(");
            return;
        }
    };
    for gid in guilds {
        if !members.containts(gid.0) {
            continue;
        };
        let name = match day {
            &Weekday::Tuesday => GUILD_DEFAULT_NAME.to_owned(),
            _ => random_name(),
        };
        let _ = set_server_name(ctx, gid.to_guild_cached(&ctx.cache).unwrap(), None, &name).await;
    }
}
