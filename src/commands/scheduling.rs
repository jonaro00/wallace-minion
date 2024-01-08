use std::{fmt::Write, str::FromStr};

use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::prelude::Message,
};

use crate::{
    database::WallaceDBClient,
    discord::{get_db_handler, get_task_signal, ScheduleTask},
};

#[group]
#[commands(tasks)]
struct Scheduling;

#[command]
#[sub_commands(add, remove)]
#[description("List all tasks in this channel.")]
async fn tasks(ctx: &Context, msg: &Message) -> CommandResult {
    let db = get_db_handler(ctx).await;
    let s = format!(
        "**Tasks in <#{}>:**\nID: (`cron schedule`) command \"argument\"\n---------------------------\n{}",
        msg.channel_id,
        db.get_all_tasks_in_channel(msg.channel_id.get())
            .await
            .map(|tasks| {
                tasks
                    .iter()
                    .fold(String::new(), |mut s, t| {
                        writeln!(
                            &mut s,
                            "{}: (`{}`) {} \"{}\"",
                            t.id,
                            t.cron,
                            t.cmd,
                            t.arg.clone().unwrap_or_default()
                        ).unwrap();
                        s
                    })
            })
            .unwrap_or_else(|e| e.to_string())
    );
    let _ = msg.channel_id.say(ctx, s).await;
    Ok(())
}

#[command]
#[min_args(2)]
#[max_args(3)]
#[required_permissions("ADMINISTRATOR")]
#[description(
    "Add a scheduled task to trigger according to a schedule.
    Use a cron schedule string in the format \"second minute hour day-of-month month day-of-week year\" (UTC based).
    Tasks with a schedule that expire are cleaned up automatically. Use the 'remove' sub-command to remove tasks.
    Availible commands: say, defaultname, randomname, lolweekly."
)]
#[usage("<cron_schedule> <command> [argument]")]
#[example(r#""0 9 20 4 10 * 2023" say "This message is sent at 8:09 PM UTC on Oct 4th 2023.""#)]
#[example(r#""0 0 8 * * Mon *" say "This message is sent every monday morning at 8 AM UTC.""#)]
#[example(r#""0 */5 * * * * *" say "This message is sent every 5th minute.""#)]
#[example(r#""0 0 0 * Jan-Jun Mon *" say "This message is sent at midnight UTC every monday in the first half of the year.""#)]
async fn add(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.quoted();
    let cron = args.current().unwrap().trim().to_owned();
    args.advance();
    let cmd = args.current().unwrap().trim().to_owned();
    args.advance();
    let arg = args.current().map(|s| s.to_owned());

    if cron::Schedule::from_str(&cron).is_err() {
        let _ = msg.channel_id.say(ctx, "Invalid cron format").await;
        return Ok(());
    }
    if cmd.parse::<ScheduleTask>().is_err() {
        let _ = msg.channel_id.say(ctx, "Invalid command to schedule").await;
        return Ok(());
    }

    let db = get_db_handler(ctx).await;
    let tx = get_task_signal(ctx).await;
    let res = db.create_task(cron, cmd, arg, msg.channel_id.get()).await;
    if res.is_ok() && tx.capacity() > 0 {
        tx.send(()).await.expect("channel to be open");
    }
    let _ = msg
        .channel_id
        .say(
            ctx,
            res.map(|_| "Added task.".to_owned())
                .unwrap_or_else(|e| e.to_string()),
        )
        .await;
    Ok(())
}

#[command]
#[num_args(1)]
#[required_permissions("ADMINISTRATOR")]
#[description("Remove a scheduled task.")]
#[usage("<task_id>")]
#[example("20")]
async fn remove(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let id = args.current().unwrap().parse::<i32>();
    if id.is_err() {
        let _ = msg.channel_id.say(ctx, "Invalid task ID").await;
        return Ok(());
    }
    let db = get_db_handler(ctx).await;
    let tx = get_task_signal(ctx).await;
    let res = db.delete_task(id.unwrap()).await;
    if res.is_ok() && tx.capacity() > 0 {
        tx.send(()).await.expect("channel to be open");
    }
    let _ = msg
        .channel_id
        .say(
            ctx,
            res.map(|_| "Removed".to_owned())
                .unwrap_or_else(|e| e.to_string()),
        )
        .await;
    Ok(())
}
