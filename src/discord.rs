use std::{
    collections::{HashMap, HashSet},
    env,
    str::FromStr,
    sync::Arc,
};

use anyhow::anyhow;
use async_openai::{config::OpenAIConfig, Client as OpenAIClient};
use async_trait::async_trait;
use chrono::Utc;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serenity::{
    client::{Client as DiscordClient, Context, EventHandler},
    framework::standard::{
        help_commands::with_embeds,
        macros::{help, hook},
        Args, BucketBuilder, CommandError, CommandGroup, CommandResult, Configuration,
        DispatchError, HelpOptions, StandardFramework,
    },
    gateway::ActivityData,
    http::Http,
    model::prelude::{ChannelId, GatewayIntents, GuildId, Message, Ready, ResumedEvent, UserId},
    prelude::TypeMapKey,
};
use songbird::Songbird;
use sqlx::PgPool;
use strum::EnumString;
use tokio::{sync::Mutex, task::JoinHandle, time::Duration};
use tracing::{error, info, warn};

use crate::{
    commands::{
        ai_voice::{WallaceAIConv, AIVOICE_GROUP},
        bank::BANK_GROUP,
        cooltext::COOLTEXT_GROUP,
        emote::EMOTE_GROUP,
        general::GENERAL_GROUP,
        riot::{lol_report, LOL_GROUP, TFT_GROUP},
        scheduling::SCHEDULING_GROUP,
        spells::{random_name, SPELLS_GROUP},
    },
    database::WallaceDBClient,
    model::Task,
    services::{riot_api::RiotAPIClients, set_server_name},
};

pub static WALLACE_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub const PREFIX: &str = "!";

pub async fn build_bot(
    discord_token: String,
    riot_token_lol: String,
    riot_token_tft: String,
    db_url: String,
    openai_token: String,
) -> DiscordClient {
    WALLACE_VERSION.get_or_init(|| {
        format!(
            "v{}{}",
            env!("CARGO_PKG_VERSION"),
            if cfg!(debug_assertions) {
                " (development)"
            } else {
                ""
            }
        )
    });

    let http = Http::new(&discord_token);
    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else if let Some(owner) = info.owner {
                owners.insert(owner.id);
            }
            match http.get_current_user().await {
                Ok(bot) => (owners, bot.id),
                Err(why) => panic!("Could not access the bot id: {:?}", why),
            }
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .unrecognised_command(unknown_command_hook)
        .after(after_hook)
        .on_dispatch_error(dispatch_error_hook)
        .bucket("slots", BucketBuilder::new_channel().delay(10))
        .await
        .group(&GENERAL_GROUP)
        .group(&AIVOICE_GROUP)
        .group(&BANK_GROUP)
        .group(&SPELLS_GROUP)
        .group(&EMOTE_GROUP)
        .group(&COOLTEXT_GROUP)
        .group(&SCHEDULING_GROUP)
        .group(&LOL_GROUP)
        .group(&TFT_GROUP)
        .help(&HELP_COMMAND);
    framework.configure(
        Configuration::new()
            .prefix(PREFIX)
            .owners(owners)
            .case_insensitivity(true)
            .on_mention(Some(bot_id)),
    );
    let songbird = Songbird::serenity();
    let client = DiscordClient::builder(
        discord_token,
        GatewayIntents::non_privileged()
            | GatewayIntents::MESSAGE_CONTENT
            | GatewayIntents::GUILD_MESSAGES,
    )
    .event_handler(Handler)
    .framework(framework)
    .voice_manager_arc(songbird.clone())
    .await
    .expect("Error creating Discord client");

    let db = sqlx::pool::Pool::connect(&db_url)
        .await
        .expect("Database connection failed.");
    info!("Connected to database!");

    // Insert shared data
    {
        // Open the data lock in write mode, so that entries can be inserted.
        let mut data = client.data.write().await;
        data.insert::<WallaceSongbird>(songbird);
        data.insert::<WallaceRiot>(Arc::new(RiotAPIClients::new(
            &riot_token_lol,
            &riot_token_tft,
        )));
        data.insert::<WallaceDB>(db);
        data.insert::<WallaceOpenAI>(Arc::new(OpenAIClient::with_config(
            OpenAIConfig::new().with_api_key(openai_token),
        )));
        data.insert::<WallaceOpenAIConvos>(Default::default());
        let (tx, rx) = tokio::sync::mpsc::channel::<()>(1);
        data.insert::<TaskSignal>(Arc::new(tx));
        data.insert::<TaskSignalRx>(rx);
    } // Release lock

    client
}

struct WallaceSongbird;
type TWallaceSongbird = Arc<Songbird>;
impl TypeMapKey for WallaceSongbird {
    type Value = TWallaceSongbird;
}
pub async fn get_songbird(ctx: &Context) -> TWallaceSongbird {
    ctx.data
        .read()
        .await
        .get::<WallaceSongbird>()
        .expect("type in typemap")
        .clone()
}

struct WallaceRiot;
type TWallaceRiot = Arc<RiotAPIClients>;
impl TypeMapKey for WallaceRiot {
    type Value = TWallaceRiot;
}
pub async fn get_riot_client(ctx: &Context) -> TWallaceRiot {
    ctx.data
        .read()
        .await
        .get::<WallaceRiot>()
        .expect("type in typemap")
        .clone()
}

struct WallaceDB;
type TWallaceDB = PgPool;
impl TypeMapKey for WallaceDB {
    type Value = TWallaceDB;
}
pub async fn get_db_handler(ctx: &Context) -> TWallaceDB {
    ctx.data
        .read()
        .await
        .get::<WallaceDB>()
        .expect("type in typemap")
        .clone()
}

struct WallaceOpenAI;
type TWallaceOpenAI = Arc<OpenAIClient<OpenAIConfig>>;
impl TypeMapKey for WallaceOpenAI {
    type Value = TWallaceOpenAI;
}
pub async fn get_openai(ctx: &Context) -> TWallaceOpenAI {
    ctx.data
        .read()
        .await
        .get::<WallaceOpenAI>()
        .expect("type in typemap")
        .clone()
}

struct WallaceOpenAIConvos;
type TWallaceOpenAIConvos = Arc<Mutex<HashMap<u64, Arc<Mutex<WallaceAIConv>>>>>;
impl TypeMapKey for WallaceOpenAIConvos {
    type Value = TWallaceOpenAIConvos;
}
pub async fn get_openai_convos(ctx: &Context) -> TWallaceOpenAIConvos {
    ctx.data
        .read()
        .await
        .get::<WallaceOpenAIConvos>()
        .expect("type in typemap")
        .clone()
}

struct TaskSignal;
type TTaskSignal = Arc<tokio::sync::mpsc::Sender<()>>;
impl TypeMapKey for TaskSignal {
    type Value = TTaskSignal;
}
pub async fn get_task_signal(ctx: &Context) -> TTaskSignal {
    ctx.data
        .read()
        .await
        .get::<TaskSignal>()
        .expect("type in typemap")
        .clone()
}
struct TaskSignalRx;
impl TypeMapKey for TaskSignalRx {
    type Value = tokio::sync::mpsc::Receiver<()>;
}

const REACTIONS: &[char] = &['😳', '😏', '😊', '😎'];
struct Handler;
#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, data: Ready) {
        info!("{} is connected!", data.user.name);
        let activity = if cfg!(debug_assertions) {
            ActivityData::playing(format!(
                "on a construction site 🔨🙂 | {}",
                WALLACE_VERSION.get().unwrap()
            ))
        } else {
            ActivityData::watching(format!(
                "you 🔨🙂 | !help | {}",
                WALLACE_VERSION.get().unwrap()
            ))
        };
        let _ = ctx.set_activity(Some(activity));
    }

    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        info!("Loaded {} guilds.", guilds.len());
        tokio::spawn(schedule_loop(ctx));
    }

    async fn resume(&self, _ctx: Context, _r: ResumedEvent) {
        info!("Reconnected.");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content.to_uppercase().contains("WALLACE")
            && msg.author != ctx.cache.current_user().to_owned().into()
        {
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
        warn!("Error in {}: {:?}", cmd_name, why);
        let _ = msg
            .channel_id
            .say(ctx, "I did a bit of an epic fail there... 😕")
            .await;
    }
}

#[hook]
async fn dispatch_error_hook(ctx: &Context, msg: &Message, err: DispatchError, cmd_name: &str) {
    if let Some(s) = match err {
        DispatchError::NotEnoughArguments { min, given } => {
            Some(format!("Need {} arguments, but only got {} 😋", min, given))
        }
        DispatchError::TooManyArguments { max, given } => Some(format!(
            "Max arguments allowed is {}, but got {} 😋",
            max, given
        )),
        DispatchError::LackingPermissions(_) | DispatchError::LackingRole => {
            Some("You can't do that 😋".to_owned())
        }
        DispatchError::OnlyForGuilds => Some("That can only be done in servers 😋".to_owned()),
        DispatchError::Ratelimited(_) => {
            let _ = msg.react(ctx, '⏱').await;
            None
        }
        _ => {
            warn!("Unhandled dispatch error in {}. {:?}", cmd_name, err);
            Some("Idk man, this seems kinda sus to me... <:AMOGUS:845281082764165131>".to_owned())
        }
    } {
        let _ = msg.channel_id.say(ctx, &s).await;
    }
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

const WEEKLY_PAYOUT: i64 = 15;
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
            info!("Time for veckopeng.");
            for u in db.get_all_users().await.expect("Could not fetch users") {
                info!("Veckopeng for {}.", u.id as u64);
                let _ = db
                    .add_bank_account_balance(u.id as u64, WEEKLY_PAYOUT)
                    .await;
            }
            info!("Veckopeng has been dealt.");
        }
    });
}

async fn schedule_loop(ctx: Context) {
    built_in_tasks(ctx.clone()).await;
    let mut running_tasks: HashMap<i32, JoinHandle<()>> = HashMap::new();
    let db = get_db_handler(&ctx).await;
    let mut rx = ctx
        .data
        .write()
        .await
        .remove::<TaskSignalRx>()
        .expect("brudda");
    loop {
        let tasks = match db.get_all_tasks().await {
            Ok(tasks) => tasks,
            Err(e) => {
                error!("Failed to get tasks: {e:?}. Retrying in 60 secs.");
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
                    if let Ok(task) = t.cmd.parse::<ScheduleTask>() {
                        for next in s.upcoming(Utc) {
                            tokio::time::sleep(
                                (next - Utc::now())
                                    .to_std()
                                    .expect("Failed time conversion"),
                            )
                            .await;
                            if task.run(&ctx, &t).await.is_err() {
                                break;
                            }
                        }
                    }
                    info!("Remove task {}", t.id);
                    if let Err(e) = db.delete_task(t.id).await {
                        warn!("Failed to remove task {}: {}", t.id, e);
                    };
                })
            });
        }
        rx.recv().await.expect("channel to be open");
        warn!("New task loop");
    }
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

impl ScheduleTask {
    pub async fn run(&self, ctx: &Context, data: &Task) -> anyhow::Result<()> {
        let db = get_db_handler(ctx).await;
        match self {
            ScheduleTask::Say => {
                let arg = match data.arg {
                    Some(ref s) => s,
                    None => return Err(anyhow!("")),
                };
                let _ = ChannelId::new(data.channel_id as u64).say(ctx, arg).await;
            }
            ScheduleTask::RandomName => {
                let g = match ctx
                    .cache
                    .channel(data.channel_id as u64)
                    .and_then(|c| c.guild(ctx))
                {
                    Some(g) => g.to_owned(),
                    None => return Err(anyhow!("")),
                };
                if let Ok((s, o)) = db.get_guild_random_names(g.id.get()).await {
                    let _ = set_server_name(ctx, g, None, &random_name(s, o)).await;
                }
            }
            ScheduleTask::DefaultName => {
                let g = match ctx
                    .cache
                    .channel(data.channel_id as u64)
                    .and_then(|c| c.guild(ctx))
                {
                    Some(g) => g.to_owned(),
                    None => return Err(anyhow!("")),
                };
                if let Ok(s) = db.get_guild_default_name(g.id.get()).await {
                    let _ = set_server_name(ctx, g, None, &s).await;
                }
            }
            ScheduleTask::LolWeekly => {
                let gc = match ctx.cache.channel(data.channel_id as u64) {
                    Some(gc) => gc.to_owned(),
                    None => return Err(anyhow!("")),
                };
                let _ = lol_report(ctx, gc).await;
            }
        };
        Ok(())
    }
}
