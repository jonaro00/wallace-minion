mod set_store;

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serenity::async_trait;
use serenity::framework::standard::macros::{command, group, hook};
use serenity::framework::standard::{
    Args, CommandError, CommandResult, DispatchError, StandardFramework,
};
use serenity::http::Http;
use serenity::model::prelude::{Activity, Guild, GuildId, Message, Ready, UserId};
use serenity::model::Timestamp;
use serenity::prelude::*;
use time::{OffsetDateTime, Weekday};
use tokio::time::{interval, Duration};

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
        .unrecognised_command(unknown_command_hook)
        .after(after_hook)
        .on_dispatch_error(dispatch_error_hook)
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
        println!("Loaded guilds! ({})", guilds.len());
        tokio::spawn(schedule_loop(ctx, guilds)).await.unwrap();
    }

    async fn ready(&self, ctx: Context, data: Ready) {
        println!("{} is connected!", data.user.name);
        let _ = ctx.set_activity(Activity::watching("you 🔨🙂")).await;
    }
}

#[hook]
async fn unknown_command_hook(ctx: &Context, msg: &Message, unknown_command_name: &str) {
    let _ = msg
        .channel_id
        .say(
            ctx,
            format!("Me not understand '{unknown_command_name}' 🤔"),
        )
        .await;
}

#[hook]
async fn after_hook(ctx: &Context, msg: &Message, cmd_name: &str, error: Result<(), CommandError>) {
    if let Err(why) = error {
        println!("[{}] Error in {}: {:?}", get_time(), cmd_name, why);
        let _ = msg
            .channel_id
            .say(ctx, "I did a bit on an epic fail there... 😕")
            .await;
    }
}

#[hook]
async fn dispatch_error_hook(ctx: &Context, msg: &Message, err: DispatchError, cmd_name: &str) {
    let s = match err {
        DispatchError::NotEnoughArguments { min, given } => {
            format!("Need {} arguments, but only got {} 😋", min, given)
        }
        DispatchError::TooManyArguments { max, given } => {
            format!("Max arguments allowed is {}, but got {} 😋", max, given)
        }
        DispatchError::LackingPermissions(_perm) => "You can't do that 😋".to_owned(),
        DispatchError::LackingRole => "You can't do that 😋".to_owned(),
        DispatchError::OnlyForGuilds => "That can only be done in servers 😋".to_owned(),
        _ => {
            println!(
                "[{}] Unhandled dispatch error in {}. {:?}",
                get_time(),
                cmd_name,
                err
            );
            "Idk man, this seems kinda sus to me... :AMOGUS:".to_owned()
        }
    };
    let _ = msg.channel_id.say(ctx, &s).await;
}

#[group]
#[commands(ping, defaultname, randomname, randomnameon, randomnameoff, bonk)]
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
    set_server_name(ctx, msg.guild(ctx).unwrap(), Some(msg), GUILD_DEFAULT_NAME).await
}

#[command]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
async fn randomname(ctx: &Context, msg: &Message) -> CommandResult {
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
#[required_permissions("ADMINISTRATOR")]
async fn randomnameon(ctx: &Context, msg: &Message) -> CommandResult {
    let mut s = SetStore::new(PathBuf::from(GUILD_FILE))?;
    s.insert(msg.guild(ctx).unwrap().id.0)?;
    msg.channel_id
        .say(ctx, "Added server to receive random names every night")
        .await?;
    Ok(())
}

#[command]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
async fn randomnameoff(ctx: &Context, msg: &Message) -> CommandResult {
    let mut s = SetStore::new(PathBuf::from(GUILD_FILE))?;
    s.remove(msg.guild(ctx).unwrap().id.0)?;
    msg.channel_id
        .say(ctx, "Removed server to receive random names every night")
        .await?;
    Ok(())
}

const TIMEOUT_LENGTH_SECONDS: i64 = 60;
#[command]
#[aliases(hammer, timeout)]
#[only_in(guilds)]
#[min_args(1)]
#[required_permissions("ADMINISTRATOR")]
// #[required_role("BigBrother")]
async fn bonk(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let gid = msg.guild_id.ok_or("Failed to get guild")?;
    loop {
        if args.is_empty() {
            break;
        }
        let arg = args.single::<String>()?;
        let uid = arg
            .strip_prefix("<@")
            .ok_or("Incorrect arg format")?
            .strip_suffix('>')
            .ok_or("Incorrect arg format")?
            .parse::<u64>()?;
            gid.edit_member(ctx, UserId(uid), |m| {
                m.disable_communication_until(
                    Timestamp::now()
                        .checked_add_signed(chrono::Duration::seconds(TIMEOUT_LENGTH_SECONDS))
                        .expect("Failed to add date")
                        .to_rfc3339(),
                )
            })
            .await?;
        let _ = msg
            .channel_id
            .say(
                ctx,
                format!(
                    "{}🔨🙂 Timed out <@{}> for {} seconds.",
                    to_cool_text("BONK!", CoolTextFont::BoldScript),
                    uid,
                    TIMEOUT_LENGTH_SECONDS
                ),
            )
            .await;
    }
    Ok(())
}

#[group]
#[commands(cooltext)]
struct CoolText;

#[command]
#[aliases(ct)]
#[sub_commands(boldfraktur, bold, bolditalic, boldscript, monospace)]
#[min_args(1)]
async fn cooltext(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, &args, CoolTextFont::BoldFraktur).await?; // default
    Ok(())
}
#[command]
#[aliases(bf)]
async fn boldfraktur(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, &args, CoolTextFont::BoldFraktur).await?;
    Ok(())
}
#[command]
#[aliases(b)]
async fn bold(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, &args, CoolTextFont::Bold).await?;
    Ok(())
}
#[command]
#[aliases(bi)]
async fn bolditalic(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, &args, CoolTextFont::BoldItalic).await?;
    Ok(())
}
#[command]
#[aliases(bs)]
async fn boldscript(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, &args, CoolTextFont::BoldScript).await?;
    Ok(())
}
#[command]
#[aliases(m)]
async fn monospace(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, &args, CoolTextFont::Monospace).await?;
    Ok(())
}
async fn do_cool_text(
    ctx: &Context,
    msg: &Message,
    args: &Args,
    font: CoolTextFont,
) -> CommandResult {
    msg.channel_id
        .say(ctx, to_cool_text(args.rest(), font))
        .await?;
    Ok(())
}
enum CoolTextFont {
    BoldFraktur,
    Bold,
    BoldItalic,
    BoldScript,
    Monospace,
}
fn cool_text_bases(font: CoolTextFont) -> Bases {
    match font {
        CoolTextFont::BoldFraktur => (Some(0x1D56C), Some(0x1D586), None),
        CoolTextFont::Bold => (Some(0x1D400), Some(0x1D41A), Some(0x1D7CE)),
        CoolTextFont::BoldItalic => (Some(0x1D468), Some(0x1D482), None),
        CoolTextFont::BoldScript => (Some(0x1D4D0), Some(0x1D4EA), None),
        CoolTextFont::Monospace => (Some(0x1D670), Some(0x1D68A), Some(0x1D7F6)),
    }
}
type Bases = (Option<u32>, Option<u32>, Option<u32>); // upper, lower, numeric
const ASCII_BASES: Bases = (Some(0x41), Some(0x61), Some(0x30));
fn to_cool_text(text: &str, font: CoolTextFont) -> String {
    let bases = cool_text_bases(font);
    let mut s = String::new();
    for c in text.chars() {
        if c.is_ascii_uppercase() && bases.0.is_some() {
            s.push(
                char::from_u32((c as u32) - ASCII_BASES.0.unwrap() + bases.0.unwrap()).unwrap_or(c),
            );
        } else if c.is_ascii_lowercase() && bases.1.is_some() {
            s.push(
                char::from_u32((c as u32) - ASCII_BASES.1.unwrap() + bases.1.unwrap()).unwrap_or(c),
            );
        } else if c.is_ascii_digit() && bases.2.is_some() {
            s.push(
                char::from_u32((c as u32) - ASCII_BASES.2.unwrap() + bases.2.unwrap()).unwrap_or(c),
            );
        } else {
            s.push(c);
        }
    }
    s
}

fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

async fn schedule_loop(ctx: Context, guilds: Vec<GuildId>) {
    let mut prev_time = get_time();
    let mut interval = interval(Duration::from_secs(60));
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
        interval.tick().await;
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
