use std::time::{UNIX_EPOCH, SystemTime};

use serenity::{
    framework::standard::{
        macros::command,
        Args,
        CommandResult
    },
    client::Context,
    model::{
        channel::{Message}, 
        id::UserId
    }
};

use crate::{util::{embeds, misc::{to_string, check_msg}, database::INTEGER}, Database, MuteCache};

#[command]
#[required_permissions(MANAGE_MESSAGES)]
#[aliases(setstreak, streak)]
async fn set_streak(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_id: u64;
    match msg.guild_id {
        Some(gid) => guild_id = gid.0,
        None => return Ok(()),
    }
    
    let arg_user = args.single::<UserId>();
    let user_id: u64;
    match arg_user {
        Ok(u) => user_id = u.0,
        Err(_) => {
            embeds::no_user(ctx, msg).await;
            return Ok(());
        }
    }

    let arg_int = args.single::<u64>();
    match arg_int {
        Ok(streak) => {
            if streak <= 16 {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

                let data = ctx.data.read().await;

                // Record in DB
                let database = data.get::<Database>().expect("Expected Database in TypeMap");


                let stunlock_table = &format!("stunlocks_{}", guild_id);
                if !database.table_exists(stunlock_table).await {
                    database.create_table(stunlock_table, &vec!["id", "streak", "streak_time", "mute_until"], &vec![INTEGER, INTEGER, INTEGER, INTEGER]).await;
                }

                if database.row_exists(stunlock_table, "id", &user_id).await {
                    database.update_int(stunlock_table, "streak", &streak, &user_id).await;
                    database.update_int(stunlock_table, "streak_time", &now, &user_id).await;
                } else {
                    database.insert_row(stunlock_table, &[&to_string(user_id), &to_string(streak), &to_string(now), &to_string(0)]).await;
                }

                // Record in Cache
                let mute_arc = data.get::<MuteCache>().expect("Expected MuteCache in TypeMap");
                let mut mute_cache = mute_arc.write().await;

                // TODO: Get rid of the FauxMessage terribleness and make it possible to set the streak of
                // users not in the database/guilds with no database.
                if let Some(guild_data) = mute_cache.get_mut(&guild_id) {
                    if let Some(user_data) = guild_data.get_mut(&user_id) {
                        user_data.streak = streak;
                        user_data.streak_time = now;
                    } else {
                        check_msg(msg.channel_id.say(&ctx.http ,"`Placeholder` You cannot currently set the streak of someone who has never been muted before.").await);
                    }
                } else {
                    check_msg(msg.channel_id.say(&ctx.http ,"`Placeholder` You cannot currently set streaks in a guild that no one has been muted in before.").await);
                }
                
                embeds::manual_streak(ctx, msg, &user_id, &streak).await;
            } else {
                embeds::streak_bad_size(ctx, msg).await;
            }
        }
        Err(_) => embeds::no_int(ctx, msg).await,
    }
    
    Ok(())
}