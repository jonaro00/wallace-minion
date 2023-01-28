use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::Message,
};

use crate::get_db_handler;

#[group]
#[commands(tasks)]
struct Scheduling;

#[command]
#[sub_commands(add, remove)]
async fn tasks(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let _ = msg
        .channel_id
        .say(
            ctx,
            db.get_all_tasks_in_channel(msg.channel_id.0)
                .await
                .and_then(|tasks| {
                    Ok(tasks
                        .iter()
                        .map(|t| {
                            format!(
                                "{}: (`{}`) {} \"{}\"\n",
                                t.id,
                                t.cron,
                                t.cmd,
                                t.arg.clone().unwrap_or("".into())
                            )
                        })
                        .collect())
                })
                .unwrap_or_else(|e| e),
        )
        .await;
    Ok(())
}

#[command]
#[min_args(2)]
#[max_args(3)]
#[required_permissions("ADMINISTRATOR")]
async fn add(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.quoted();
    let cron = args.current().unwrap().trim().to_owned();
    args.advance();
    let cmd = args.current().unwrap().trim().to_owned();
    args.advance();
    let arg = args.current().map(|s| s.to_owned());

    if crontab::Crontab::parse(&cron).is_err() {
        let _ = msg.channel_id.say(ctx, "Invalid cron format").await;
        return Ok(());
    }
    let cmds = ["randomname", "say"];
    if !cmds.contains(&cmd.as_str()) {
        let _ = msg.channel_id.say(ctx, "Invalid command to schedule").await;
        return Ok(());
    }

    let db = get_db_handler(ctx).await;
    let _ = msg
        .channel_id
        .say(
            ctx,
            db.create_task(cron, cmd, arg, msg.channel_id.0)
                .await
                .map(|_| "Added")
                .unwrap_or_else(|e| e),
        )
        .await;
    Ok(())
}

#[command]
#[num_args(1)]
#[required_permissions("ADMINISTRATOR")]
async fn remove(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let id = args.current().unwrap().parse::<i32>();
    if id.is_err() {
        let _ = msg.channel_id.say(ctx, "Invalid task ID").await;
        return Ok(());
    }
    let db = get_db_handler(ctx).await;
    let _ = msg
        .channel_id
        .say(
            ctx,
            db.delete_task(id.unwrap())
                .await
                .map(|_| "Removed")
                .unwrap_or_else(|e| e),
        )
        .await;
    Ok(())
}
