use std::{time::{SystemTime, UNIX_EPOCH}, sync::Arc, collections::HashMap};

use serenity::{
    client::Context,
    model::{
        permissions::Permissions, 
        channel::{
            PermissionOverwrite,
            PermissionOverwriteType
        },
        prelude::UserId, 
        id::ChannelId,
    }
};

use crate::{Database, MuteCache, events::on_message::FauxMessage,};

use super::{embeds, misc::to_string, database::INTEGER};

pub async fn check_loop(ctx: Arc<Context>) {
    let data = ctx.data.read().await;
    let mute_arc = data.get::<MuteCache>().expect("Expected MuteCache in TypeMap");
    let mut mute_cache = mute_arc.write().await;
    for (gid, guild_data) in &mut *mute_cache {
        for (uid, author_data) in guild_data {
            author_data.update(&ctx, gid, uid).await;
        }
    }
}

trait MutePermissions {
    fn mute(user_id: u64) -> Vec<Self> where Self: Sized;
    fn unmute(user_id: u64) -> Vec<Self> where Self: Sized;
}

impl MutePermissions for PermissionOverwrite {
    fn mute(user_id: u64) -> Vec<PermissionOverwrite> where PermissionOverwrite: Sized {
        vec![PermissionOverwrite {
            allow: Permissions::empty(),
            deny: Permissions::ADD_REACTIONS
            .union(Permissions::SEND_MESSAGES),
            kind: PermissionOverwriteType::Member(UserId(user_id)),
        },
        ]
    }

    fn unmute(user_id: u64) -> Vec<PermissionOverwrite> where PermissionOverwrite: Sized {
        vec![PermissionOverwrite {
            allow: Permissions::empty(),
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(UserId(user_id)),
        },
        ]
    }
}

#[derive(Clone, Copy)]
pub struct MuteInfo {
    pub streak: u64,
    pub streak_time: u64,
    pub mute_until: u64,
}

impl MuteInfo {
    pub async fn new_mute(ctx: &Context, msg: &FauxMessage) -> Self {
        let mut a = Self {
            streak: 0,
            streak_time: 0,
            mute_until: 0,
        };
        a.mute(ctx, msg).await;
        a
    } 

    async fn update(&mut self, ctx: &Context, guild_id: &u64, author_id: &u64) {
        let data = ctx.data.read().await;
        let database = data.get::<Database>().expect("Expected Database in TypeMap");
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let stunlock_table = &format!("stunlocks_{}", guild_id);

        // Unmute User
        if now >= self.mute_until {
            for cid in database.get_all_rows(&format!("channels_{}", guild_id), "id").await {
                if let Ok(channel) = ChannelId(cid).to_channel(&ctx.http).await {
                    if let Some(mut guild_channel) = channel.guild() {
                        // Update values
                        database.update_int(stunlock_table, "mute_until", &(i64::MAX as u64), author_id).await;
                        self.mute_until = i64::MAX as u64;

                        // Update perms
                        let _ = guild_channel.edit(&ctx.http, |c| {
                            c.permissions(MutePermissions::unmute(*author_id))
                        }).await;
                        
                        // Notify user
                        if let Ok(user) = UserId(*author_id).to_user(&ctx.http).await {
                            embeds::unmute(ctx, &user, guild_id).await;
                        }
                    }
                }
            }
        }

        // Lower streak
        if self.streak > 0 && now - 21600 >= self.streak_time {
            let elapsed = now.saturating_sub(self.streak_time);
            let decrease_by = elapsed / 21600;
            let new_streak = self.streak.saturating_sub(decrease_by);
            let new_streak_time = self.streak_time + (decrease_by * 21600);

            if new_streak != self.streak || self.streak == 0 {
                if self.streak == 0 {
                    // Drop info on a user if they no longer have a streak
                    database.delete_row(stunlock_table, "id", author_id).await;
                    
                    let data = ctx.data.read().await;
                    let mute_arc = data.get::<MuteCache>().expect("Expected MuteCache in TypeMap");
                    let mut mute_cache = mute_arc.write().await;
                    mute_cache.entry(*guild_id).and_modify(|guild_data| {
                        guild_data.remove(author_id);
                    });
                } else {
                    database.update_int(stunlock_table, "streak", &new_streak, author_id).await;
                    database.update_int(stunlock_table, "streak_time", &new_streak_time, author_id).await;

                    self.streak_time = new_streak_time;
                    self.streak = new_streak;
                }
            }
        }
    }

    async fn mute(&mut self, ctx: &Context, msg: &FauxMessage) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // Increase the user's streak
        let new_streak = self.streak + 1;
        let duration = 2u64.pow(2 * new_streak as u32 - 1);
        let new_mute_until = now + duration;

        let data = ctx.data.read().await;
        let database = data.get::<Database>().expect("Expected Database in TypeMap");

        // Record
        let author_id = &msg.author.id.0;
        let stunlock_table = &format!("stunlocks_{}", &msg.guild_id);
        if !database.table_exists(stunlock_table).await {
            database.create_table(stunlock_table, &vec!["id", "streak", "streak_time", "mute_until"], &vec![INTEGER, INTEGER, INTEGER, INTEGER]).await;
        }

        if database.row_exists(stunlock_table, "id", author_id).await {
            database.update_int(stunlock_table, "streak", &new_streak, author_id).await;
            database.update_int(stunlock_table, "streak_time", &now, author_id).await;
            database.update_int(stunlock_table, "mute_until", &new_mute_until, author_id).await;
        } else {
            database.insert_row(stunlock_table, &[&to_string(author_id), &to_string(new_streak), &to_string(now), &to_string(new_mute_until)]).await;
        }
        
        self.streak = new_streak;
        self.mute_until = new_mute_until;
        self.streak_time = now;

        // Update perms
        for cid in database.get_all_rows(&format!("channels_{}", msg.guild_id), "id").await {
            if let Ok(channel) = ChannelId(cid).to_channel(&ctx.http).await {
                if let Some(mut guild_channel) = channel.guild() {
                    match guild_channel.edit(&ctx.http, |c| {
                        c.permissions(MutePermissions::mute(*author_id))
                    }).await {
                        Err(why) => println!("{:?}", why),
                        _ => {},
                    }
                }
            }
        }

        // Send a message
        embeds::stunlock(&ctx, msg, duration, new_streak).await;
    } 
}

pub async fn mute(ctx: &Context, msg: &FauxMessage, guild_id: u64) {
    let data = ctx.data.read().await;
    let mute_arc = data.get::<MuteCache>().expect("Expected MuteCache in TypeMap");
    let mut mute_cache = mute_arc.write().await;
    let author_id = msg.author.id.0;

    if let Some(guild_data) = mute_cache.get_mut(&guild_id) {
        // Data present for the guild
        if let Some(author_data) = guild_data.get_mut(&author_id) {
            // Data present for the user
            author_data.mute(ctx, msg).await;
        } else {
            // No data present for the user
            let author_data = MuteInfo::new_mute(ctx, msg).await;
            guild_data.insert(author_id, author_data);
        }
    } else {
        // No data present for the guild or user
        let mut guild_data = HashMap::new();
        let author_data = MuteInfo::new_mute(ctx, msg).await;
        guild_data.insert(author_id, author_data);
        mute_cache.insert(guild_id, guild_data);
    }
}