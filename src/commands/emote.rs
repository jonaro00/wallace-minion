use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::Message,
};

use crate::services::seven_tv::get_emote_png_gif_url;

#[group]
#[commands(emote)]
struct Emote;

#[command]
#[aliases(e)]
#[min_args(1)]
#[description(
    "Search and post an emote from 7TV. Use quotes for an exact search match. Use an emote id for a specific emote."
)]
#[usage("<search_string|emote_id>")]
#[example("xdd")]
#[example("\"DogLookingSussyAndCold\"")]
#[example("60edf43ba60faa2a91cfb082")]
async fn emote(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let q = args.current().unwrap();
    let typing = ctx.http.start_typing(msg.channel_id.0);
    let emote_url = get_emote_png_gif_url(q)
        .await
        .unwrap_or_else(|e| e.to_string());
    if let Ok(typing) = typing {
        let _ = typing.stop();
    }
    msg.channel_id.say(ctx, emote_url).await?;
    Ok(())
}
