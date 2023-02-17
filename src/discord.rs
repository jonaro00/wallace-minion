use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::{collections::HashSet, str::FromStr};

use chrono::Utc;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serenity::{
    async_trait,
    client::{Client as DiscordClient, Context, EventHandler},
    framework::standard::{
        help_commands::with_embeds,
        macros::{help, hook},
        Args, CommandError, CommandGroup, CommandResult, DispatchError, HelpOptions,
        StandardFramework,
    },
    http::Http,
    model::prelude::{
        Activity, ChannelId, GatewayIntents, GuildId, Message, Ready, ResumedEvent, UserId,
    },
    prelude::TypeMapKey,
};
use strum::EnumString;
use tokio::{task::JoinHandle, time::Duration};

use crate::{
    commands::{
        bank::BANK_GROUP,
        cooltext::COOLTEXT_GROUP,
        emote::EMOTE_GROUP,
        general::GENERAL_GROUP,
        riot::{lol_report, LOL_GROUP, TFT_GROUP},
        scheduling::SCHEDULING_GROUP,
        spells::{random_name, SPELLS_GROUP},
    },
    database::WallaceDBClient,
    prisma::{new_client_with_url, PrismaClient},
    services::{riot_api::RiotAPIClients, set_server_name},
};

pub fn wallace_version() -> String {
    format!(
        "v{}{}",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) {
            " (development)"
        } else {
            ""
        },
    )
}

pub const PREFIX: &str = "!";

pub async fn build_bot(
    discord_token: String,
    riot_token_lol: String,
    riot_token_tft: String,
    db_url: String,
) -> DiscordClient {
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
        .configure(|c| c.prefix(PREFIX).owners(owners))
        .unrecognised_command(unknown_command_hook)
        .after(after_hook)
        .on_dispatch_error(dispatch_error_hook)
        .group(&GENERAL_GROUP)
        .group(&BANK_GROUP)
        .group(&SPELLS_GROUP)
        .group(&EMOTE_GROUP)
        .group(&COOLTEXT_GROUP)
        .group(&SCHEDULING_GROUP)
        .group(&LOL_GROUP)
        .group(&TFT_GROUP)
        .help(&HELP_COMMAND);
    let client = DiscordClient::builder(
        discord_token,
        GatewayIntents::non_privileged()
            | GatewayIntents::MESSAGE_CONTENT
            | GatewayIntents::GUILD_MESSAGES,
    )
    .event_handler(Handler)
    .framework(framework)
    .await
    .expect("Error creating Discord client");

    let db = new_client_with_url(&db_url)
        .await
        .expect("Database connection failed.");
    println!("Connected to database!");

    // Insert shared data
    {
        // Open the data lock in write mode, so that entries can be inserted.
        let mut data = client.data.write().await;
        data.insert::<WallaceRiot>(Arc::new(RiotAPIClients::new(
            &riot_token_lol,
            &riot_token_tft,
        )));
        data.insert::<WallaceDB>(Arc::new(db));
    } // Release lock

    client
}

struct WallaceRiot;
impl TypeMapKey for WallaceRiot {
    type Value = Arc<RiotAPIClients>;
}
pub async fn get_riot_client(ctx: &Context) -> Arc<RiotAPIClients> {
    ctx.data
        .read()
        .await
        .get::<WallaceRiot>()
        .expect("Expected Riot Client in TypeMap.")
        .clone()
}

struct WallaceDB;
impl TypeMapKey for WallaceDB {
    type Value = Arc<PrismaClient>;
}
pub async fn get_db_handler(ctx: &Context) -> Arc<PrismaClient> {
    ctx.data
        .read()
        .await
        .get::<WallaceDB>()
        .expect("Expected DB Handler in TypeMap.")
        .clone()
}

const REACTIONS: &[char] = &['😳', '😏', '😊', '😎'];
struct Handler;
#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, data: Ready) {
        println!("{} is connected!", data.user.name);
        let activity = if cfg!(debug_assertions) {
            Activity::playing(format!(
                "on a construction site 🔨🙂 | {}",
                wallace_version()
            ))
        } else {
            Activity::watching(format!("you 🔨🙂 | !help | {}", wallace_version()))
        };
        let _ = ctx.set_activity(activity).await;
    }

    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        println!("Loaded {} guilds.", guilds.len());
        tokio::spawn(schedule_loop(ctx));
    }

    async fn resume(&self, _ctx: Context, _r: ResumedEvent) {
        println!("Reconnected.");
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
        println!("Error in {}: {:?}", cmd_name, why);
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
        DispatchError::LackingPermissions(_) | DispatchError::LackingRole => {
            "You can't do that 😋".to_owned()
        }
        DispatchError::OnlyForGuilds => "That can only be done in servers 😋".to_owned(),
        _ => {
            println!("Unhandled dispatch error in {}. {:?}", cmd_name, err);
            "Idk man, this seems kinda sus to me... <:AMOGUS:845281082764165131>".to_owned()
        }
    };
    let _ = msg.channel_id.say(ctx, &s).await;
}

#[help]
#[embed_success_colour("#389d58")]
#[max_levenshtein_distance(2)]
async fn help_command(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

const WEEKLY_PAYOUT: i64 = 6;
async fn built_in_tasks(ctx: Context) {
    let db = get_db_handler(&ctx).await;
    // Weekly payout
    tokio::spawn(async move {
        let s = cron::Schedule::from_str("0 0 8 * * Mon *").unwrap();
        for next in s.upcoming(Utc) {
            tokio::time::sleep(
                (next - Utc::now())
                    .to_std()
                    .expect("Failed time conversion"),
            )
            .await;
            println!("Time for veckopeng.");
            for u in db.get_all_users().await.expect("Could not fetch users") {
                let _ = db
                    .add_bank_account_balance(u.id as u64, WEEKLY_PAYOUT)
                    .await;
            }
            println!("Veckopeng has been dealt.");
        }
    });
}

#[derive(EnumString)]
pub enum ScheduleTask {
    #[strum(serialize = "say")]
    Say,
    #[strum(serialize = "defaultname")]
    DefaultName,
    #[strum(serialize = "randomname")]
    RandomName,
    #[strum(serialize = "lolweekly")]
    LolWeekly,
}

async fn schedule_loop(ctx: Context) {
    built_in_tasks(ctx.clone()).await;
    let mut running_tasks: HashMap<i32, JoinHandle<()>> = HashMap::new();
    let db = get_db_handler(&ctx).await;
    loop {
        let tasks = match db.get_all_tasks().await {
            Ok(tasks) => tasks,
            Err(e) => {
                println!("Failed to get tasks: {e:?}. Cancelling task loop.");
                tokio::time::sleep(Duration::from_secs(60)).await;
                continue;
            }
        };
        let mut db_task_ids: HashSet<i32> = HashSet::new();
        for t in &tasks {
            db_task_ids.insert(t.id);
        }
        for rt in &running_tasks {
            if !db_task_ids.contains(rt.0) {
                rt.1.abort();
            }
        }
        for t in tasks.into_iter() {
            running_tasks.entry(t.id).or_insert_with(|| {
                let ctx = ctx.clone();
                let db = db.clone();
                tokio::spawn(async move {
                    let s = cron::Schedule::from_str(&t.cron).expect("Invalid cron string");
                    for next in s.upcoming(Utc) {
                        tokio::time::sleep(
                            (next - Utc::now())
                                .to_std()
                                .expect("Failed time conversion"),
                        )
                        .await;
                        let task = t.cmd.parse::<ScheduleTask>();
                        if task.is_err() {
                            break;
                        }
                        match task.unwrap() {
                            ScheduleTask::Say => {
                                let arg = match t.arg {
                                    Some(ref s) => s,
                                    None => break,
                                };
                                let _ = ChannelId(t.channel_id as u64).say(&ctx, arg).await;
                            }
                            ScheduleTask::RandomName => {
                                let g = match ctx
                                    .cache
                                    .channel(t.channel_id as u64)
                                    .and_then(|c| c.guild())
                                    .and_then(|g| g.guild(&ctx))
                                {
                                    Some(g) => g,
                                    None => break,
                                };
                                if let Ok((s, o)) = db.get_guild_random_names(g.id.0).await {
                                    let _ =
                                        set_server_name(&ctx, g, None, &random_name(s, o)).await;
                                }
                            }
                            ScheduleTask::DefaultName => {
                                let g = match ctx
                                    .cache
                                    .channel(t.channel_id as u64)
                                    .and_then(|c| c.guild())
                                    .and_then(|g| g.guild(&ctx))
                                {
                                    Some(g) => g,
                                    None => break,
                                };
                                if let Ok(s) = db.get_guild_default_name(g.id.0).await {
                                    let _ = set_server_name(&ctx, g, None, &s).await;
                                }
                            }
                            ScheduleTask::LolWeekly => {
                                let channel = ChannelId(t.channel_id as u64)
                                    .to_channel(&ctx)
                                    .await
                                    .map_err(|_| "what")
                                    .and_then(|c| c.guild().ok_or("no guild"));
                                if channel.is_err() {
                                    println!("failed lolweekly");
                                    continue;
                                }
                                let _ = lol_report(&ctx, channel.unwrap()).await;
                            }
                        }
                    }
                    if let Err(e) = db.delete_task(t.id).await {
                        println!("WARN: Failed to remove task {}: {}", t.id, e);
                    };
                })
            });
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
