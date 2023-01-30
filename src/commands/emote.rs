use serenity::{
    builder::CreateEmbed,
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::Message,
};

use crate::services::seven_tv::get_emote_name_url;

#[group]
#[commands(emote)]
struct Emote;

#[command]
#[aliases(e)]
#[min_args(1)]
#[description(
    "Search and post one or more emotes from 7TV. Use quotes for an exact search match. Use an emote id for a specific emote."
)]
#[usage("<search_string|emote_id...> [- rest of message]")]
#[example("xdd")]
#[example("\"DogLookingSussyAndCold\"")]
#[example("SNIFFA xdding - Haha these are funny :D")]
#[example("60edf43ba60faa2a91cfb082")]
async fn emote(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let typing = ctx.http.start_typing(msg.channel_id.0);
    let mut embeds = vec![];
    while let Some(q) = args.current() {
        if q == "-" {
            break;
        }
        let mut e = CreateEmbed::default();
        match get_emote_name_url(q).await {
            Ok((name, url)) => {
                e.description(&name).image(&url);
            }
            Err(err) => {
                e.description(format!("{q}: {err}"));
            }
        }
        embeds.push(e);
        args.advance();
    }
    if let Ok(typing) = typing {
        let _ = typing.stop();
    }
    let _ = msg
        .channel_id
        .send_message(ctx, |m| m.add_embeds(embeds))
        .await;
    Ok(())
}
