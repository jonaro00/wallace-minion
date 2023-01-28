use std::collections::BTreeMap;

use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::Message,
};

const WEEKLY_REPORT_MEMBERS_FILE: &str = "weekly_report_members.json";
type LoLAccount = (String, String);
type AccountList = Vec<LoLAccount>;
type GuildWeeklyReportMembers = BTreeMap<String, AccountList>;
type WeeklyReportMembers = BTreeMap<u64, GuildWeeklyReportMembers>;

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
#[sub_commands(add, remove)]
#[description("Show weekly playtime for every summoner added with 'add'.")]
async fn weekly(ctx: &Context, msg: &Message) -> CommandResult {
    lol_report(ctx, msg.channel_id).await
}
#[command]
#[min_args(2)]
#[description("Add a summoner to the weekly report every Monday morning.")]
#[usage("<name> <summoners...>")]
#[example("Me \"EUNE:MupDef Crispy\" \"EUW:WallaceBigBrain\"")]
async fn add(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let store = JsonStore::new(WEEKLY_REPORT_MEMBERS_FILE);
    let mut m: WeeklyReportMembers = store.read().map_err(|_| "Failed to read members.")?;
    let channel = msg.channel_id.0;
    let member = args.current().unwrap().to_owned();
    m.entry(channel)
        .or_insert_with(GuildWeeklyReportMembers::new);
    let cm = m.get_mut(&channel).unwrap();
    cm.entry(member.clone()).or_insert_with(AccountList::new);
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
    if let Err(err) = store.write(&Some(m)).map_err(|_| "Failed to save file.") {
        let _ = msg
            .channel_id
            .say(ctx, format!("Failed to add accounts: {err}"))
            .await;
    }
    Ok(())
}
#[command]
#[num_args(1)]
#[description(
    "Remove all summoners associated with a name from the weekly report every Monday morning."
)]
#[usage("<name>")]
#[example("Me")]
async fn remove(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let store = JsonStore::new(WEEKLY_REPORT_MEMBERS_FILE);
    let mut m: WeeklyReportMembers = store.read().map_err(|_| "Failed to read members.")?;
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
            if cm.is_empty() {
                m.remove_entry(&channel);
            }
            v.len()
        }
    };
    if let Err(err) = store.write(&Some(m)).map_err(|_| "Failed to save file.") {
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
    let m: WeeklyReportMembers = JsonStore::new(WEEKLY_REPORT_MEMBERS_FILE)
        .read()
        .map_err(|_| "Failed to read members.")?;
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
    for (member, accounts) in cm {
        s.push_str(&format!("**{member}**:\n"));
        for (ser, name) in accounts {
            let server = match PlatformRoute::from_str(ser) {
                Ok(o) => o,
                Err(err) => {
                    s.push_str(&format!("{ser}: {err}\n"));
                    continue;
                }
            };
            s = push_playtime_str(s, &client, server, name).await;
        }
    }
    if s.is_empty() {
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
    let typing = ctx.http.start_typing(msg.channel_id.0);
    let puuid_tft = &client
        .get_summoner_tft(server, &name)
        .await
        .map_err(|e| e.to_string())
        .and_then(|o| o.ok_or_else(|| "Summoner not found".to_owned()))?
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
    .into()
}
