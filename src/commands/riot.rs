use std::str::FromStr;

use riven::consts::PlatformRoute;
use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    futures::StreamExt,
    model::prelude::{GuildChannel, Message},
    utils::parse_user_mention,
};

use crate::{
    database::WallaceDBClient,
    discord::{get_db_handler, get_riot_client},
    services::riot_api::RiotAPIClients,
};

#[group]
#[commands(lol)]
struct LoL;

#[command]
#[sub_commands(playtime, weekly)]
#[description("LoL+TFT playtime.")]
async fn lol(_ctx: &Context, _msg: &Message, mut _args: Args) -> CommandResult {
    Err(Box::new(serenity::Error::Other("Not implemented")))
}

#[command]
#[only_in(guilds)]
#[sub_commands(add, remove)]
#[description("Show weekly playtime for every summoner added with 'add'.")]
async fn weekly(ctx: &Context, msg: &Message) -> CommandResult {
    lol_report(ctx, msg.channel(ctx).await?.guild().ok_or("no guild")?).await
}

#[command]
#[min_args(2)]
#[description("Add summoners to the weekly report.")]
#[usage("<user> <summoners...>")]
#[example("@jonaro00 \"EUNE:MupDef Crispy\" \"EUW:WallaceBigBrain\"")]
async fn add(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let user = args.current().unwrap().to_owned();
    let target_uid = parse_user_mention(&user).ok_or("Invalid user tag")?.get();
    args.advance();
    for arg in args.quoted().iter::<String>().filter_map(|s| s.ok()) {
        let (server, summoner) = match parse_server_summoner(&arg) {
            Ok(pair) => pair,
            Err(err) => {
                let _ = msg
                    .channel_id
                    .say(ctx, format!("Couldn't add {arg}: {err}"))
                    .await;
                return Ok(());
            }
        };
        let _ = msg
            .channel_id
            .say(
                ctx,
                match db
                    .create_lol_account(server.clone(), summoner.clone(), target_uid)
                    .await
                {
                    Ok(_) => format!("Adding [{server}] {summoner} to {user}."),
                    Err(err) => format!("Couldn't add {arg}: {err}"),
                },
            )
            .await;
    }
    Ok(())
}
#[command]
#[num_args(1)]
#[description("Remove all summoners associated with a user from the weekly report.")]
#[usage("<user>")]
#[example("@jonaro00")]
async fn remove(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let user = args.current().unwrap();
    let target_uid = parse_user_mention(user).ok_or("Invalid user tag")?.get();
    for acc in db.get_all_lol_accounts_in_user(target_uid).await? {
        let _ = msg
            .channel_id
            .say(
                ctx,
                match db
                    .delete_lol_account(acc.server.clone(), acc.summoner.clone())
                    .await
                {
                    Ok(_) => format!("Removed [{}] {}", acc.server, acc.summoner),
                    Err(err) => format!(
                        "Failed to remove [{}] {}: {}",
                        acc.server, acc.summoner, err
                    ),
                },
            )
            .await;
    }
    Ok(())
}

async fn push_playtime_str(
    mut s: String,
    client: &RiotAPIClients,
    server: PlatformRoute,
    name: &str,
) -> String {
    let region = server.to_regional();
    let puuid_lol = match client
        .get_summoner_lol(server, name)
        .await
        .map_err(|e| e.to_string())
        .and_then(|o| o.ok_or_else(|| "Summoner not found".to_owned()))
    {
        Ok(a) => a,
        Err(e) => {
            s.push_str(&format!(
                "Couldn't find summmoner {} on {}: {}\n",
                name, server, e
            ));
            return s;
        }
    }
    .puuid;
    let puuid_tft = match client
        .get_summoner_tft(server, name)
        .await
        .map_err(|e| e.to_string())
        .and_then(|o| o.ok_or_else(|| "Summoner not found".to_owned()))
    {
        Ok(a) => a,
        Err(e) => {
            s.push_str(&format!(
                "Couldn't find summmoner {} on {}: {}\n",
                name, server, e
            ));
            return s;
        }
    }
    .puuid;
    let (amount, secs) = match client.get_playtime(region, &puuid_lol, &puuid_tft).await {
        Ok(p) => p,
        Err(e) => {
            s.push_str(&format!(
                "Failed to get playtime for {name} on {server}: {e}\n",
            ));
            return s;
        }
    };
    let emoji = is_sus(&secs);
    let (hrs, mins, secs) = seconds_to_hms(secs);
    s.push_str(&format!(
        "[{server}] {name}: {amount} games, {hrs}h{mins}m{secs}s {emoji}\n",
    ));
    s
}

#[command]
#[aliases(pt)]
#[min_args(1)]
#[description("Calculate LoL+TFT playtime for summoner(s).")]
#[usage("<summoners...>")]
#[example("\"EUNE:MupDef Crispy\" \"EUW:WallaceBigBrain\"")]
async fn playtime(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let client = get_riot_client(ctx).await;
    let typing = ctx.http.start_typing(msg.channel_id);
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
    typing.stop();
    msg.channel_id.say(ctx, s).await?;
    Ok(())
}

pub async fn lol_report(ctx: &Context, gc: GuildChannel) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let client = get_riot_client(ctx).await;
    let mut s = String::from("**Weekly playtime:**\n");
    let mut mem = gc.guild_id.members_iter(ctx).boxed();
    let typing = ctx.http.start_typing(gc.id);
    while let Some(Ok(m)) = mem.next().await {
        if let Ok(v) = db.get_all_lol_accounts_in_user(m.user.id.get()).await {
            if v.is_empty() {
                continue;
            }
            s.push_str(&format!("**{}**:\n", m.nick.unwrap_or(m.user.name)));
            for acc in v {
                let server = match PlatformRoute::from_str(&acc.server) {
                    Ok(o) => o,
                    Err(err) => {
                        s.push_str(&format!("{}: {err}\n", acc.server));
                        continue;
                    }
                };
                s = push_playtime_str(s, &client, server, &acc.summoner).await;
            }
        }
    }
    typing.stop();
    gc.id.say(ctx, s).await?;
    Ok(())
}

#[group]
#[commands(tft)]
struct TFT;

#[command]
#[sub_commands(analysis)]
#[description("TFT meta analysis.")]
async fn tft(_ctx: &Context, _msg: &Message, mut _args: Args) -> CommandResult {
    Err(Box::new(serenity::Error::Other("Not implemented")))
}

#[command]
#[num_args(1)]
#[description("Calculate TFT stats for the current set.")]
#[usage("<summoner>")]
#[example("\"EUW:Thebausffs\"")]
async fn analysis(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let client = get_riot_client(ctx).await;
    let arg = args.current().unwrap().to_owned();
    let (server, name) = parse_server_summoner(&arg)
        .and_then(|(ser, nam)| Ok((PlatformRoute::from_str(&ser)?, nam)))?;
    let typing = ctx.http.start_typing(msg.channel_id);
    let puuid_tft = &client
        .get_summoner_tft(server, &name)
        .await
        .map_err(|e| e.to_string())
        .and_then(|o| o.ok_or_else(|| "Summoner not found".to_owned()))?
        .puuid;
    let ss = client.tft_analysis(server.to_regional(), puuid_tft).await?;
    typing.stop();
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
    .into()
}
