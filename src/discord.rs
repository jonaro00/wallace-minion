use std::{
    collections::{HashMap, HashSet},
    env,
    num::NonZeroU64,
    str::FromStr,
    sync::Arc,
};

use anyhow::anyhow;
use async_openai::Client as OpenAIClient;
use async_trait::async_trait;
use aws_sdk_comprehend::Client as ComprehendClient;
use aws_sdk_polly::Client as PollyClient;
use chrono::Utc;
use lazy_static::lazy_static;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serenity::{
    client::{Client as DiscordClient, Context, EventHandler},
    framework::standard::{
        buckets::LimitedFor,
        help_commands::with_embeds,
        macros::{help, hook},
        Args, CommandError, CommandGroup, CommandResult, DispatchError, HelpOptions,
        StandardFramework,
    },
    gateway::ActivityData,
    http::Http,
    model::prelude::{ChannelId, GatewayIntents, GuildId, Message, Ready, ResumedEvent, UserId},
    prelude::TypeMapKey,
};
use songbird::Songbird;
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
    prisma::{self, new_client_with_url, PrismaClient},
    services::{riot_api::RiotAPIClients, set_server_name},
};

lazy_static! {
    pub static ref WALLACE_VERSION: String = format!(
        "v{}{}",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) {
            " (development)"
        } else {
            ""
        },
    );
}

pub const PREFIX: &str = "!";

pub async fn build_bot(
    discord_token: String,
    riot_token_lol: String,
    riot_token_tft: String,
    db_url: String,
    openai_token: String,
    aws_config: aws_types::SdkConfig,
) -> DiscordClient {
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
        .bucket("slots", |b| b.delay(10).limit_for(LimitedFor::Channel))
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
    framework.configure(|c| {
        c.prefix(PREFIX)
            .owners(owners)
            .case_insensitivity(true)
            .on_mention(Some(bot_id))
    });
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

    let db = new_client_with_url(&db_url)
        .await
        .expect("Database connection failed.");
    info!("Connected to database!");

    let polly_client = PollyClient::new(&aws_config);
    let comprehend_client = ComprehendClient::new(&aws_config);

    // Insert shared data
    {
        // Open the data lock in write mode, so that entries can be inserted.
        let mut data = client.data.write().await;
        data.insert::<WallaceSongbird>(songbird);
        data.insert::<WallaceRiot>(Arc::new(RiotAPIClients::new(
            &riot_token_lol,
            &riot_token_tft,
        )));
        data.insert::<WallaceDB>(Arc::new(db));
        data.insert::<WallaceOpenAI>(Arc::new(Mutex::new((
            OpenAIClient::new().with_api_key(openai_token),
            Default::default(),
        ))));
        data.insert::<WallacePolly>(Arc::new(polly_client));
        let (tx, rx) = tokio::sync::mpsc::channel::<()>(1);
        data.insert::<TaskSignal>(Arc::new(tx));
        data.insert::<TaskSignalRx>(rx);
        data.insert::<WallaceComprehend>(Arc::new(comprehend_client));
    } // Release lock

    client
}

struct WallaceSongbird;
impl TypeMapKey for WallaceSongbird {
    type Value = Arc<Songbird>;
}
pub async fn get_songbird(ctx: &Context) -> Arc<Songbird> {
    ctx.data
        .read()
        .await
        .get::<WallaceSongbird>()
        .expect("Expected Songbird in TypeMap.")
        .clone()
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

struct WallaceOpenAI;
impl TypeMapKey for WallaceOpenAI {
    type Value = Arc<Mutex<(OpenAIClient, HashMap<u64, Arc<Mutex<WallaceAIConv>>>)>>;
}
pub async fn get_openai(
    ctx: &Context,
) -> Arc<Mutex<(OpenAIClient, HashMap<u64, Arc<Mutex<WallaceAIConv>>>)>> {
    ctx.data
        .read()
        .await
        .get::<WallaceOpenAI>()
        .expect("Expected OpenAI Client in TypeMap.")
        .clone()
}

struct WallacePolly;
impl TypeMapKey for WallacePolly {
    type Value = Arc<PollyClient>;
}
pub async fn get_polly(ctx: &Context) -> Arc<PollyClient> {
    ctx.data
        .read()
        .await
        .get::<WallacePolly>()
        .expect("Expected AWS Polly Client in TypeMap.")
        .clone()
}
struct WallaceComprehend;
impl TypeMapKey for WallaceComprehend {
    type Value = Arc<ComprehendClient>;
}
pub async fn get_comprehend(ctx: &Context) -> Arc<ComprehendClient> {
    ctx.data
        .read()
        .await
        .get::<WallaceComprehend>()
        .expect("Expected AWS Comprehend Client in TypeMap.")
        .clone()
}

struct TaskSignal;
impl TypeMapKey for TaskSignal {
    type Value = Arc<tokio::sync::mpsc::Sender<()>>;
}
pub async fn get_task_signal(ctx: &Context) -> Arc<tokio::sync::mpsc::Sender<()>> {
    ctx.data
        .read()
        .await
        .get::<TaskSignal>()
        .expect("Expected Task mpsc Sender in TypeMap.")
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
                *WALLACE_VERSION
            ))
        } else {
            ActivityData::watching(format!("you 🔨🙂 | !help | {}", *WALLACE_VERSION))
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
    pub async fn run(&self, ctx: &Context, data: &prisma::task::Data) -> anyhow::Result<()> {
        let db = get_db_handler(ctx).await;
        match self {
            ScheduleTask::Say => {
                let arg = match data.arg {
                    Some(ref s) => s,
                    None => return Err(anyhow!("")),
                };
                let _ = ChannelId(NonZeroU64::new(data.channel_id as u64).expect("not 0"))
                    .say(ctx, arg)
                    .await;
            }
            ScheduleTask::RandomName => {
                let g = match ctx
                    .cache
                    .channel(data.channel_id as u64)
                    .and_then(|c| c.guild())
                    .and_then(|g| g.guild(ctx))
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
                    .and_then(|c| c.guild())
                    .and_then(|g| g.guild(ctx))
                {
                    Some(g) => g.to_owned(),
                    None => return Err(anyhow!("")),
                };
                if let Ok(s) = db.get_guild_default_name(g.id.get()).await {
                    let _ = set_server_name(ctx, g, None, &s).await;
                }
            }
            ScheduleTask::LolWeekly => {
                let gc = match ctx
                    .cache
                    .channel(data.channel_id as u64)
                    .and_then(|c| c.guild())
                {
                    Some(gc) => gc,
                    None => return Err(anyhow!("")),
                };
                let _ = lol_report(ctx, gc).await;
            }
        };
        Ok(())
    }
}
