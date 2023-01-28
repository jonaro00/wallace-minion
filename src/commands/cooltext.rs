use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::Message,
};

use crate::services::cool_text::{to_cool_text, Font};

#[group]
#[commands(cooltext)]
struct CoolText;

#[command]
#[aliases(ct)]
#[sub_commands(boldfraktur, bold, bolditalic, boldscript, monospace)]
#[min_args(1)]
#[description("Make some cool text in one of a few different fonts.")]
#[usage("[font] <text>")]
#[example("bf Hello there!")]
async fn cooltext(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    boldfraktur(ctx, msg, args).await
}
#[command]
#[aliases(bf)]
async fn boldfraktur(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, args, Font::BoldFraktur).await
}
#[command]
#[aliases(b)]
async fn bold(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, args, Font::Bold).await
}
#[command]
#[aliases(bi)]
async fn bolditalic(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, args, Font::BoldItalic).await
}
#[command]
#[aliases(bs)]
async fn boldscript(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, args, Font::BoldScript).await
}
#[command]
#[aliases(m)]
async fn monospace(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    do_cool_text(ctx, msg, args, Font::Monospace).await
}
async fn do_cool_text(ctx: &Context, msg: &Message, args: Args, font: Font) -> CommandResult {
    msg.channel_id
        .say(ctx, to_cool_text(args.rest(), font))
        .await?;
    Ok(())
}
