use rand::{rngs::StdRng, Rng, SeedableRng};
use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::{Guild, Message},
    utils::parse_username,
};

use crate::services::bonk_user;

// const GUILD_FILE: &str = "name_change_guilds.txt";

// const _KORV_INGVAR_ANNIVERSARY_NAME: &str = "KorvIngvars Följeskaps Klubb";
// const KORV_INGVAR_ANNIVERSARIES: &[&(Month, u8, &str)] = &[
//     &(
//         Month::September,
//         13,
//         "Idag minns vi dagen då KorvIngvar föddes <:IngvarDrip:931696495412011068>🥳🎉",
//     ),
//     &(
//         Month::September,
//         23,
//         "Idag minns vi dagen då KorvIngvar dog <:IngvarDrip:931696495412011068>✝😞",
//     ),
// ];

const GUILD_DEFAULT_NAME: &str = "Tisdags Gaming Klubb";

const GUILD_NAME_SUBJECTS: &[&str] = &[
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
    "Höjdarna",
    "Muminpappa",
    "LillaMy",
    "Lipton",
    "Gordon",
    "Wallace",
    "Gromit",
    "Berit",
    "Herbert",
    "KorvIngvar",
    "Knugen",
    "EggMan",
    "Trump",
    "Obama",
    "Steffe",
    "Svergie",
    "MupDef",
    "Kina",
    "NordKorea",
    "GustavVasa",
    "Trollface",
    "Pepe",
    "MackaPacka",
    "PostisPer",
    "StoraMaskiner",
    "Svampbob",
    "Perry",
    "DrDoofenshmirtz",
    "PostNord",
    "ICA",
    "MrBeast",
    "TheBausffs",
    "Gragas",
    "Rammus",
    "Notch",
    "EdwardBlom",
    "LeifGWPersson",
    "Mauri",
    "ElonMusk",
    "JohnCena",
    "MrBean",
];
const GUILD_NAME_OBJECTS: &[&str] = &[
    "Gaming",
    "Minecraft",
    "Fortnite",
    "LoL",
    "TFT",
    "Gartic",
    "AmongUs",
    "Terraria",
    "MarioKart",
    "SmashBros",
    "Roblox",
    "Pokémon",
    "Magic",
    "LEGO",
    "Schack",
    "Agario",
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
    "LivsGlädje",
    "👺",
    "Anime",
    "Kpop",
    "Musik",
    "Bok",
    "Golf",
    "Fotbolls",
    "Matematik",
    "Programmerings",
    "Politik",
    "Plugg",
    "Kubb",
    "Nörd",
    "Fika",
    "Hatt",
    "Pingvin",
    "Välfärds",
    "Ekonomi",
    "Ondskefulla",
    "Hemliga",
    "Trädgårds",
    "Pepega",
    "Shilling",
    "BOMBA",
    "Boomer",
];

#[group]
#[commands(
    ping,
    version,
    power,
    delete,
    gamba,
    bonk,
    defaultname,
    randomname,
    // randomnameon,
    // randomnameoff
)]
struct General;

#[command]
#[description("Challenge me to a game of table tennis! (and check if I'm alive)")]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    if msg.author.id.0 == 224233166024474635 {
        let _ = msg.react(ctx, '👑').await;
    }
    let _ = tokio::join!(msg.react(ctx, '👍'), msg.channel_id.say(ctx, "Pong!"),);
    Ok(())
}

#[command]
#[description("Check my IQ! (output is in semver format)")]
async fn version(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .say(
            ctx,
            format!(
                "v{}{}",
                env!("CARGO_PKG_VERSION"),
                if cfg!(debug_assertions) {
                    " (development)"
                } else {
                    ""
                },
            ),
        )
        .await?;
    Ok(())
}

#[command]
#[description("Censor me. Reply to a message from me with this command to delete it.")]
async fn delete(ctx: &Context, msg: &Message) -> CommandResult {
    let _ = msg.delete(ctx).await;
    if let Some(r) = msg.referenced_message.as_deref() {
        if r.author.id == ctx.cache.current_user().id {
            r.delete(ctx).await?;
        }
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
#[num_args(1)]
#[description(
    "Summon mods in chat to start the GAMBA. Tag a user and they might get bonked. Or you."
)]
#[usage("<user>")]
#[example("@Yxaria")]
async fn gamba(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut rng: StdRng = SeedableRng::from_entropy();
    let uid = parse_username(args.current().unwrap()).ok_or_else(|| "Invalid user tag")?;
    if rng.gen_ratio(1, 5) {
        // Win
        bonk_user(ctx, msg, uid).await?;
    } else {
        // Loss
        bonk_user(ctx, msg, msg.author.id.0).await?;
    }
    Ok(())
}

#[command]
#[aliases(hammer, timeout)]
#[only_in(guilds)]
#[min_args(1)]
#[required_permissions("ADMINISTRATOR")]
#[description("Bonk a user.")]
async fn bonk(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    for arg in args.iter::<String>().map(|a| a.unwrap()) {
        let uid = parse_username(&arg).ok_or_else(|| "Invalid user tag")?;
        bonk_user(ctx, msg, uid).await?;
    }
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
#[description("Set the server name to the default.")]
#[only_in(guilds)]
async fn defaultname(ctx: &Context, msg: &Message) -> CommandResult {
    set_server_name(ctx, msg.guild(ctx).unwrap(), Some(msg), GUILD_DEFAULT_NAME).await
}

#[command]
#[only_in(guilds)]
#[required_permissions("ADMINISTRATOR")]
#[description("Set the server name to a random one.")]
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

// #[command]
// #[only_in(guilds)]
// #[required_permissions("ADMINISTRATOR")]
// #[description("Turn on random server name every day.")]
// async fn randomnameon(ctx: &Context, msg: &Message) -> CommandResult {
//     let mut s = SetStore::new(GUILD_FILE)?;
//     s.insert(msg.guild(ctx).unwrap().id.0)?;
//     msg.channel_id
//         .say(ctx, "Added server to receive random names every night")
//         .await?;
//     Ok(())
// }

// #[command]
// #[only_in(guilds)]
// #[required_permissions("ADMINISTRATOR")]
// #[description("Turn on random server name every day.")]
// async fn randomnameoff(ctx: &Context, msg: &Message) -> CommandResult {
//     let mut s = SetStore::new(GUILD_FILE)?;
//     s.remove(msg.guild(ctx).unwrap().id.0)?;
//     msg.channel_id
//         .say(ctx, "Removed server to receive random names every night")
//         .await?;
//     Ok(())
// }
