use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use chrono::Duration as cDuration;
use itertools::Itertools;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use riven::consts::PlatformRoute;
use serenity::async_trait;
use serenity::client::{Client as DiscordClient, Context, EventHandler};
use serenity::framework::standard::macros::{command, group, hook};
use serenity::framework::standard::{
    Args, CommandError, CommandResult, DispatchError, StandardFramework,
};
use serenity::http::Http;
use serenity::model::prelude::{
    Activity, ChannelId, GatewayIntents, Guild, GuildId, Message, Ready, ResumedEvent, Timestamp,
    UserId,
};
use serenity::prelude::TypeMapKey;
use time::{OffsetDateTime, Weekday};
use tokio::time::{interval, Duration as tDuration};

use wallace_minion::riot_api::RiotAPIClient;
use wallace_minion::set_store::SetStore;

const GUILD_FILE: &str = "name_change_guilds.txt";

const GUILD_DEFAULT_NAME: &str = "Tisdags Gaming Klubb";

const GUILD_NAME_SUBJECTS: [&str; 36] = [
    "Tisdag",
    "Johan",
    "Matteus",
    "Daniel",
    "Gabriel",
    "Mattias",
    "Olle",
    "Wilmer",
    "Vincent",
    "Habibi",
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
    "Kina",
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

struct RiotClient;
impl TypeMapKey for RiotClient {
    type Value = Arc<RiotAPIClient>;
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let discord_token =
        env::var("DISCORD_TOKEN").expect("Discord token missing! (env variable `DISCORD_TOKEN`)");
    let riot_token_lol = env::var("RIOT_TOKEN_LOL")
        .expect("Riot token for LoL missing! (env variable `RIOT_TOKEN_LOL`)");
    let riot_token_tft = env::var("RIOT_TOKEN_TFT")
        .expect("Riot token for TFT missing! (env variable `RIOT_TOKEN_TFT`)");

    let riot_api = RiotAPIClient::new(&riot_token_lol, &riot_token_tft);

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
        .group(&LOL_GROUP)
        .group(&TFT_GROUP);
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
        data.insert::<RiotClient>(Arc::new(riot_api));
    } // Release lock

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {why:?}");
    }
}

const REACTIONS: [char; 4] = ['😳', '😏', '😊', '😎'];
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

    async fn resume(&self, _ctx: Context, _r: ResumedEvent) {
        println!("Reconnected!");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content.to_uppercase().contains("WALLACE") {
            let mut rng: StdRng = SeedableRng::from_entropy();
            let _ = msg
                .react(ctx, REACTIONS[rng.gen_range(0..REACTIONS.len())])
                .await;
        }
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
            .say(ctx, "I did a bit of an epic fail there... 😕")
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
            "Idk man, this seems kinda sus to me... <:AMOGUS:845281082764165131>".to_owned()
        }
    };
    let _ = msg.channel_id.say(ctx, &s).await;
}

#[group]
#[commands(
    ping,
    power,
    defaultname,
    randomname,
    randomnameon,
    randomnameoff,
    bonk
)]
struct General;

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    if msg.author.id.0 == 224233166024474635 {
        let _ = msg.react(ctx, '👑').await;
    }
    let _ = tokio::join!(msg.react(ctx, '👍'), msg.channel_id.say(ctx, "Pong!"),);
    Ok(())
}

#[command]
async fn power(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .say(ctx, "https://youtu.be/wjr6LKJOyxY")
        .await?;
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
                    .checked_add_signed(cDuration::seconds(TIMEOUT_LENGTH_SECONDS))
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

const WEEKLY_REPORT_MEMBERS_FILE: &str = "weekly_report_members.json";
type LoLAccount = (String, String);
type AccountList = Vec<LoLAccount>;
type GuildWeeklyReportMembers = HashMap<String, AccountList>;
type WeeklyReportMembers = HashMap<u64, GuildWeeklyReportMembers>;
fn load_members() -> Result<WeeklyReportMembers, String> {
    let p = Path::new(WEEKLY_REPORT_MEMBERS_FILE);
    if !p.is_file() {
        std::fs::write(p, "{}").unwrap();
    }
    std::fs::read_to_string(p)
        // .map(|s| if s == "" { "{}".to_owned() } else { s })
        .map_err(|err| format!("Failed to read file: {err}"))
        .and_then(|s| {
            serde_json::from_str::<WeeklyReportMembers>(&s)
                .map_err(|err| format!("Failed to parse JSON: {err}"))
        })
}
fn save_members(m: WeeklyReportMembers) -> Result<(), String> {
    serde_json::to_string_pretty::<WeeklyReportMembers>(&m)
        .map_err(|err| format!("Failed to convert to JSON: {err}"))
        .and_then(|s| {
            std::fs::write(WEEKLY_REPORT_MEMBERS_FILE, s)
                .map_err(|err| format!("Failed to write file: {err}"))
        })
}

#[group]
#[commands(lol)]
struct LoL;

#[command]
#[sub_commands(playtime, weekly)]
async fn lol(_ctx: &Context, _msg: &Message, mut _args: Args) -> CommandResult {
    Err(Box::new(serenity::Error::Other("Not implemented")))
}
#[command]
#[sub_commands(add, remove)]
async fn weekly(ctx: &Context, msg: &Message) -> CommandResult {
    lol_report(ctx, msg.channel_id).await
}
#[command]
#[min_args(2)]
async fn add(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let mut m = load_members()?;
    let channel = msg.channel_id.0;
    let member = args.current().unwrap().to_owned();
    if !m.contains_key(&channel) {
        m.insert(channel.clone(), GuildWeeklyReportMembers::new());
    }
    let cm = m.get_mut(&channel).unwrap();
    if !cm.contains_key(&member) {
        cm.insert(member.clone(), AccountList::new());
    }
    args.advance();
    for arg in args.quoted().iter::<String>().filter_map(|s| s.ok()) {
        let (server, name) = match parse_server_summoner(&arg) {
            Ok(pair) => pair,
            Err(err) => {
                let _ = msg
                    .channel_id
                    .say(ctx, format!("Couldn't add {arg}: {err}"))
                    .await;
                return Ok(());
            }
        };
        cm.get_mut(&member)
            .unwrap()
            .push((server.clone(), name.clone()));
        let _ = msg
            .channel_id
            .say(ctx, format!("Adding [{server}] {name} to {member}."))
            .await;
    }
    if let Err(err) = save_members(m) {
        let _ = msg
            .channel_id
            .say(ctx, format!("Failed to add accounts: {err}"))
            .await;
    }
    Ok(())
}
#[command]
#[num_args(1)]
async fn remove(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut m = load_members()?;
    let member = args.current().unwrap().to_owned();
    let channel = msg.channel_id.0;
    let cm = match m.get_mut(&channel) {
        None => {
            let _ = msg
                .channel_id
                .say(ctx, format!("No members registered in <#{channel}>"))
                .await;
            return Ok(());
        }
        Some(cm) => cm,
    };
    let num = match cm.remove_entry(&member) {
        None => {
            let _ = msg
                .channel_id
                .say(ctx, format!("Didn't find member {member}"))
                .await;
            return Ok(());
        }
        Some((_, v)) => {
            if cm.len() == 0 {
                m.remove_entry(&channel);
            }
            v.len()
        }
    };
    if let Err(err) = save_members(m) {
        let _ = msg
            .channel_id
            .say(ctx, format!("Failed to remove member: {err}"))
            .await;
    } else {
        let _ = msg
            .channel_id
            .say(ctx, format!("Removed {member} ({num} accounts)"))
            .await;
    }
    Ok(())
}

async fn get_riot_client(ctx: &Context) -> Arc<RiotAPIClient> {
    let data_read = ctx.data.read().await;
    data_read
        .get::<RiotClient>()
        .expect("Expected Riot Client in TypeMap.")
        .clone()
}

async fn push_playtime_str(
    mut s: String,
    client: &RiotAPIClient,
    server: PlatformRoute,
    name: &str,
) -> String {
    let region = server.to_regional();
    let puuid_lol = match client
        .get_summoner_lol(server, &name)
        .await
        .map_err(|e| e.to_string())
        .and_then(|o| o.ok_or("Summoner not found".to_owned()))
    {
        Ok(a) => a,
        Err(e) => {
            s.push_str(&format!(
                "Couldn't find summmoner {} on {}: {}\n",
                name,
                server.to_string(),
                e
            ));
            return s;
        }
    }
    .puuid;
    let puuid_tft = match client
        .get_summoner_tft(server, &name)
        .await
        .map_err(|e| e.to_string())
        .and_then(|o| o.ok_or("Summoner not found".to_owned()))
    {
        Ok(a) => a,
        Err(e) => {
            s.push_str(&format!(
                "Couldn't find summmoner {} on {}: {}\n",
                name,
                server.to_string(),
                e
            ));
            return s;
        }
    }
    .puuid;
    let (amount, secs) = match client.get_playtime(region, &puuid_lol, &puuid_tft).await {
        Ok(p) => p,
        Err(e) => {
            s.push_str(&format!(
                "Failed to get playtime for {} on {}: {}\n",
                name,
                server.to_string(),
                e
            ));
            return s;
        }
    };
    let emoji = is_sus(&secs);
    let (hrs, mins, secs) = seconds_to_hms(secs);
    s.push_str(&format!(
        "[{}] {name}: {amount} games, {hrs}h{mins}m{secs}s {emoji}\n",
        server.to_string()
    ));
    s
}

#[command]
#[aliases(pt)]
#[min_args(1)]
async fn playtime(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let client = get_riot_client(ctx).await;
    let typing = ctx.http.start_typing(msg.channel_id.0);
    let mut s = String::from("**Weekly playtime:**\n");
    for arg in args.quoted().iter::<String>().filter_map(|s| s.ok()) {
        let (server, name) = match parse_server_summoner(&arg)
            .and_then(|(ser, nam)| Ok((PlatformRoute::from_str(&ser)?, nam)))
        {
            Ok(o) => o,
            Err(err) => {
                s.push_str(&format!("{arg}: {err}\n"));
                continue;
            }
        };
        s = push_playtime_str(s, &client, server, &name).await;
    }
    if let Ok(typing) = typing {
        let _ = typing.stop();
    }
    msg.channel_id.say(ctx, s).await?;
    Ok(())
}

async fn lol_report(ctx: &Context, channel: ChannelId) -> CommandResult {
    let client = get_riot_client(ctx).await;
    let mut s = String::from("**Weekly playtime:**\n");
    let m = load_members()?;
    let cid = channel.0;
    let cm = match m.get(&cid) {
        None => {
            let _ = channel
                .say(ctx, format!("No members registered in <#{cid}>"))
                .await;
            return Ok(());
        }
        Some(cm) => cm,
    };
    let typing = ctx.http.start_typing(cid);
    for (member, accounts) in cm.iter().sorted() {
        s.push_str(&format!("**{member}**:\n"));
        for (ser, name) in accounts {
            let server = match PlatformRoute::from_str(&ser) {
                Ok(o) => o,
                Err(err) => {
                    s.push_str(&format!("{ser}: {err}\n"));
                    continue;
                }
            };
            s = push_playtime_str(s, &client, server, &name).await;
        }
    }
    if s.len() == 0 {
        s.push_str("No members 😥");
    }
    if let Ok(typing) = typing {
        let _ = typing.stop();
    }
    channel.say(ctx, s).await?;
    Ok(())
}

#[group]
#[commands(tft)]
struct TFT;

#[command]
#[sub_commands(analysis)]
async fn tft(_ctx: &Context, _msg: &Message, mut _args: Args) -> CommandResult {
    Err(Box::new(serenity::Error::Other("Not implemented")))
}

#[command]
#[min_args(1)]
async fn analysis(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let client = get_riot_client(ctx).await;
    let arg = args.current().unwrap().to_owned();
    let (server, name) = parse_server_summoner(&arg)
        .and_then(|(ser, nam)| Ok((PlatformRoute::from_str(&ser)?, nam)))?;
    let typing = ctx.http.start_typing(msg.channel_id.0);
    let puuid_tft = &client
        .get_summoner_tft(server, &name)
        .await
        .map_err(|e| e.to_string())
        .and_then(|o| o.ok_or("Summoner not found".to_owned()))?
        .puuid;
    let ss = client.tft_analysis(server.to_regional(), puuid_tft).await?;
    if let Ok(typing) = typing {
        let _ = typing.stop();
    }
    for s in ss {
        msg.channel_id.say(ctx, s).await?;
    }
    Ok(())
}

fn parse_server_summoner(
    s: &str,
) -> Result<(String, String), Box<dyn std::error::Error + Sync + Send>> {
    match s.trim_matches('"').split_once(':') {
        None => Err("Incorrect format".to_owned())?,
        Some((server, name)) => Ok((server.to_owned(), name.to_owned())),
    }
}

fn seconds_to_hms(mut secs: i64) -> (i64, i64, i64) {
    let hrs = secs / 3600;
    secs -= 3600 * hrs;
    let mins = secs / 60;
    secs -= 60 * mins;
    (hrs, mins, secs)
}

fn is_sus(secs: &i64) -> String {
    if *secs > 3600 * 10 {
        "<:AMOGUS:845281082764165131>"
    } else if *secs > 3600 * 5 {
        "🤨"
    } else if *secs > 3600 * 2 {
        "😐"
    } else if *secs > 0 {
        "🙂"
    } else {
        ""
    }
    .to_owned()
}

fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

async fn schedule_loop(ctx: Context, guilds: Vec<GuildId>) {
    let mut prev_time = get_time();
    let mut interval = interval(tDuration::from_secs(60));
    let mut do_weekly: bool = false;
    loop {
        let time = get_time();
        let day = time.weekday();
        let hour = time.hour();
        let minute = time.minute();
        if day != prev_time.weekday() {
            println!("It's a new day ({day}) :D");
            if day == Weekday::Monday {
                do_weekly = true;
            }
            nightly_name_update(&ctx, &guilds, &day).await;
        }
        if hour != prev_time.hour() {
            if do_weekly && hour == 8 {
                do_weekly = false;
                weekly_lol_report(&ctx).await;
            }
        }
        if minute != prev_time.minute() {}
        prev_time = time;
        interval.tick().await;
    }
}

async fn nightly_name_update(ctx: &Context, guilds: &Vec<GuildId>, day: &Weekday) {
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
        let name = match *day {
            Weekday::Tuesday => GUILD_DEFAULT_NAME.to_owned(),
            _ => random_name(),
        };
        let _ = set_server_name(ctx, gid.to_guild_cached(&ctx.cache).unwrap(), None, &name).await;
    }
}

async fn weekly_lol_report(ctx: &Context) {
    let m = load_members().unwrap();
    for cid in m.keys() {
        let _ = lol_report(ctx, ChannelId(*cid)).await;
    }
}
