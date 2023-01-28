use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::{collections::HashSet, str::FromStr};

use chrono::Utc;
use database::DBHandler;
use rand::{rngs::StdRng, Rng, SeedableRng};
use sea_orm::Database;
use serenity::model::prelude::ChannelId;
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
    model::prelude::{Activity, GatewayIntents, GuildId, Message, Ready, ResumedEvent, UserId},
    prelude::TypeMapKey,
};
use time::OffsetDateTime;
use tokio::task::JoinHandle;
use tokio::time::Duration;

mod commands;
mod database;
mod services;

use crate::commands::general::{random_name, GUILD_DEFAULT_NAME};
use crate::services::set_server_name;
use crate::{
    commands::{
        cooltext::COOLTEXT_GROUP, emote::EMOTE_GROUP, general::GENERAL_GROUP,
        scheduling::SCHEDULING_GROUP,
    },
    services::riot_api::RiotAPIClients,
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let discord_token =
        env::var("DISCORD_TOKEN").expect("Discord token missing! (env variable `DISCORD_TOKEN`)");
    let riot_token_lol = env::var("RIOT_TOKEN_LOL")
        .expect("Riot token for LoL missing! (env variable `RIOT_TOKEN_LOL`)");
    let riot_token_tft = env::var("RIOT_TOKEN_TFT")
        .expect("Riot token for TFT missing! (env variable `RIOT_TOKEN_TFT`)");
    let db_url =
        env::var("DATABASE_URL").expect("URL for database missing! (env variable `DATABASE_URL`)");

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
        .group(&EMOTE_GROUP)
        .group(&COOLTEXT_GROUP)
        .group(&SCHEDULING_GROUP)
        // .group(&LOL_GROUP)
        // .group(&TFT_GROUP)
        .help(&HELP_COMMAND);
    let mut client = DiscordClient::builder(
        discord_token,
        GatewayIntents::non_privileged()
            | GatewayIntents::MESSAGE_CONTENT
            | GatewayIntents::GUILD_MESSAGES,
    )
    .event_handler(Handler)
    .framework(framework)
    .await
    .expect("Error creating Discord client");

    let db = Database::connect(db_url)
        .await
        .expect("Database connection failed.");
    println!("Connected to database!");
    let dbh = DBHandler { db };

    // Insert shared data
    {
        // Open the data lock in write mode, so that entries can be inserted.
        let mut data = client.data.write().await;
        data.insert::<WallaceRiot>(Arc::new(RiotAPIClients::new(
            &riot_token_lol,
            &riot_token_tft,
        )));
        data.insert::<WallaceDB>(Arc::new(dbh));
    } // Release lock

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {why:?}");
    }
}

struct WallaceRiot;
impl TypeMapKey for WallaceRiot {
    type Value = Arc<RiotAPIClients>;
}
async fn get_riot_client(ctx: &Context) -> Arc<RiotAPIClients> {
    let data_read = ctx.data.read().await;
    data_read
        .get::<WallaceRiot>()
        .expect("Expected Riot Client in TypeMap.")
        .clone()
}

struct WallaceDB;
impl TypeMapKey for WallaceDB {
    type Value = Arc<database::DBHandler>;
}
async fn get_db_handler(ctx: &Context) -> Arc<DBHandler> {
    let data_read = ctx.data.read().await;
    data_read
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
            Activity::playing("on a construction site 🔨🙂")
        } else {
            Activity::watching("you 🔨🙂 | !help")
        };
        let _ = ctx.set_activity(activity).await;
    }

    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        println!("Loaded {} guilds.", guilds.len());
        tokio::spawn(schedule_loop(ctx)).await.unwrap();
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
        DispatchError::LackingPermissions(_) | DispatchError::LackingRole => {
            "You can't do that 😋".to_owned()
        }
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

#[help]
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

fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

async fn schedule_loop(ctx: Context) {
    let mut running_tasks: HashMap<i32, JoinHandle<()>> = HashMap::new();
    loop {
        let db = get_db_handler(&ctx).await;
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
                    println!("Starting task {}", t.id);
                    let s = cron::Schedule::from_str(&t.cron).expect("Invalid cron string");
                    for next in s.upcoming(Utc) {
                        tokio::time::sleep(
                            (next - Utc::now())
                                .to_std()
                                .expect("Failed time conversion"),
                        )
                        .await;
                        match t.cmd.as_str() {
                            "say" => {
                                println!("Task {} say", t.id);
                                let arg = match t.arg {
                                    Some(ref s) => s,
                                    None => break,
                                };
                                let _ = ChannelId(t.channel_id as u64).say(&ctx, arg).await;
                            }
                            "randomname" => {
                                println!("Task {} randomname", t.id);
                                let g = match ctx
                                    .cache
                                    .channel(t.channel_id as u64)
                                    .and_then(|c| c.guild())
                                    .and_then(|g| g.guild(&ctx))
                                {
                                    Some(g) => g,
                                    None => break,
                                };
                                let _ = set_server_name(&ctx, g, None, &random_name()).await;
                            }
                            "defaultname" => {
                                println!("Task {} defaultname", t.id);
                                let g = match ctx
                                    .cache
                                    .channel(t.channel_id as u64)
                                    .and_then(|c| c.guild())
                                    .and_then(|g| g.guild(&ctx))
                                {
                                    Some(g) => g,
                                    None => break,
                                };
                                let _ = set_server_name(&ctx, g, None, GUILD_DEFAULT_NAME).await;
                            }
                            _ => (),
                        }
                    }
                    println!("Task {} ended. Removing.", t.id);
                    if let Err(e) = db.delete_task(t.id).await {
                        println!("WARN: Failed to remove task {}: {}", t.id, e);
                    };
                })
            });
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

// async fn weekly_lol_report(ctx: &Context) {
//     let m: WeeklyReportMembers = match JsonStore::new(WEEKLY_REPORT_MEMBERS_FILE).read() {
//         Ok(m) => m,
//         Err(_) => {
//             return;
//         }
//     };
//     for cid in m.keys() {
//         let _ = lol_report(ctx, ChannelId(*cid)).await;
//     }
// }
