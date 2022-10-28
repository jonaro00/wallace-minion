mod set_store;

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Duration as cDuration;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client as HttpClient, ClientBuilder, Url};
use serde::Deserialize;
use serenity::async_trait;
use serenity::client::{Client as DiscordClient, Context, EventHandler};
use serenity::framework::standard::macros::{command, group, hook};
use serenity::framework::standard::{
    Args, CommandError, CommandResult, DispatchError, StandardFramework,
};
use serenity::http::Http;
use serenity::model::prelude::{
    Activity, GatewayIntents, Guild, GuildId, Message, Ready, Timestamp, UserId,
};
use serenity::prelude::TypeMapKey;
use time::{OffsetDateTime, Weekday};
use tokio::time::{interval, Duration as tDuration};

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

struct RiotToken;
impl TypeMapKey for RiotToken {
    type Value = Arc<String>;
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let discord_token =
        env::var("DISCORD_TOKEN").expect("Discord token missing! (env variable `DISCORD_TOKEN`)");
    let riot_token =
        env::var("RIOT_TOKEN").expect("Riot token missing! (env variable `RIOT_TOKEN`)");

    let http = Http::new(&discord_token);
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
        .group(&COOLTEXT_GROUP)
        .group(&LOL_GROUP);
    let mut client = DiscordClient::builder(
        discord_token,
        GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT,
    )
    .event_handler(Handler)
    .framework(framework)
    .await
    .expect("Error creating client");

    // Insert shared data
    {
        // Open the data lock in write mode, so keys can be inserted to it.
        let mut data = client.data.write().await;
        data.insert::<RiotToken>(Arc::new(riot_token));
    } // Release lock

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

#[derive(Debug, Deserialize)]
struct SummonerDTO {
    // accountId: String,
    // profileIconId: i32,
    // revisionDate: i64,
    // name: String,
    // id: String,
    puuid: String,
    // summonerLevel: i64,
}
#[derive(Debug, Deserialize)]
struct MatchDTO {
    info: InfoDTO,
}
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct InfoDTO {
    gameDuration: i64,
}
#[derive(Debug, Deserialize)]
struct TFTMatchDTO {
    info: TFTInfoDTO,
}
#[derive(Debug, Deserialize)]
struct TFTInfoDTO {
    // game_length: f32,
    participants: Vec<TFTParticipantDTO>,
}
#[derive(Debug, Deserialize)]
struct TFTParticipantDTO {
    puuid: String,
    time_eliminated: f32,
}

type MatchesList = Vec<String>;

#[group]
#[commands(lol)]
struct LoL;

#[command]
#[sub_commands(playtime)]
async fn lol(_ctx: &Context, _msg: &Message, _args: Args) -> CommandResult {
    Err(Box::new(serenity::Error::Other("Not implemented")))
}
#[command]
#[aliases(pt)]
#[min_args(1)]
async fn playtime(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let riot_token = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<RiotToken>()
            .expect("Expected RiotToken in TypeMap.")
            .clone()
    };
    let typing = ctx.http.start_typing(msg.channel_id.0);
    let mut headers = HeaderMap::new();
    headers.insert("X-Riot-Token", HeaderValue::from_str(&riot_token).unwrap());
    let client = ClientBuilder::new()
        .default_headers(headers)
        .build()
        .unwrap();
    let mut s = String::new();
    for arg in args.quoted().iter::<String>().filter_map(|s| s.ok()) {
        let a = arg.trim_matches('"').split_once(':');
        if a.is_none() {
            s.push_str(&format!("Incorrect format: {arg}\n"));
            continue;
        }
        let (server, name) = a.unwrap();
        let puuid = match get_summoner(&client, server, name).await {
            Ok(SummonerDTO { puuid }) => puuid,
            Err(err) => {
                s.push_str(&format!(
                    "Couldn't find summmoner {name} on {server}: {err}\n"
                ));
                continue;
            }
        };
        let (amount, mut secs) = get_playtime(&client, &puuid).await?;
        let hrs = secs / 3600;
        secs -= 3600 * hrs;
        let mins = secs / 60;
        secs -= 60 * mins;
        s.push_str(&format!("[{server}] {name} played {amount} games in the past week. Total: {hrs}h{mins}m{secs}s. 🤨\n"));
    }
    if let Ok(typing) = typing {
        let _ = typing.stop();
    }
    msg.channel_id.say(ctx, s).await?;
    Ok(())
}

async fn get_summoner(
    client: &HttpClient,
    server: &str,
    summoner_name: &str,
) -> CommandResult<SummonerDTO> {
    let serv_id = match server {
        "EUW" => "euw1",
        "EUNE" => "eun1",
        _ => return Err(Box::new(serenity::Error::Other("Invalid server"))),
    };
    Ok(client
        .get(
            Url::parse(&format!(
            "https://{serv_id}.api.riotgames.com/lol/summoner/v4/summoners/by-name/{summoner_name}"
            ))
            .unwrap(),
        )
        .send()
        .await?
        .json::<SummonerDTO>()
        .await?)
}

async fn get_playtime(client: &HttpClient, puuid: &str) -> CommandResult<(usize, i64)> {
    let now = Timestamp::now();
    let then = now.checked_sub_signed(cDuration::weeks(1)).unwrap();
    let nowts = now.timestamp();
    let thents = then.timestamp();
    let mut secs = 0;
    let lol_matches = client.get(
        format!(
            "https://europe.api.riotgames.com/lol/match/v5/matches/by-puuid/{puuid}/ids?startTime={thents}&endTime={nowts}&start=0&count=100"
        ))
        .send()
        .await?
        .json::<MatchesList>()
        .await?;
    for m_id in &lol_matches {
        let mtch = client
            .get(format!(
                "https://europe.api.riotgames.com/lol/match/v5/matches/{m_id}"
            ))
            .send()
            .await?
            .json::<MatchDTO>()
            .await?;
        secs += mtch.info.gameDuration;
    }
    let tft_matches = client
        .get(format!(
            "https://europe.api.riotgames.com/tft/match/v1/matches/by-puuid/{puuid}/ids?startTime={thents}&endTime={nowts}&start=0&count=100"
        ))
        .send()
        .await?
        .json::<MatchesList>()
        .await?;
    for m_id in &tft_matches {
        let mtch = client
            .get(format!(
                "https://europe.api.riotgames.com/tft/match/v1/matches/{m_id}"
            ))
            .send()
            .await?
            .json::<TFTMatchDTO>()
            .await?;
        secs += mtch
            .info
            .participants
            .iter()
            .find(|p| p.puuid == puuid)
            .unwrap()
            .time_eliminated as i64;
    }
    Ok((lol_matches.len() + tft_matches.len(), secs))
}

fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

async fn schedule_loop(ctx: Context, guilds: Vec<GuildId>) {
    let mut prev_time = get_time();
    let mut interval = interval(tDuration::from_secs(60));
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
