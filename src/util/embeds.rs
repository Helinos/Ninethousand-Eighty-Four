use std::{
    sync::Arc,
    time::Duration,
};

use serenity::{
    client::Context,
    utils::Color,
    model::{
        channel::Message,
        prelude::User,
        id::GuildId,
    },
    Result as SerenityResult,
};

use crate::{events::on_message::FauxMessage, Database};

use super::misc::{seconds_to_string, check_msg, to_string};

const DEFAULT_COLOR: Color = Color::from_rgb(149, 165, 166);
const ERROR_COLOR: Color = Color::from_rgb(231, 76, 60);
const SETTINGS_COLOR: Color = Color::from_rgb(13, 71, 161);


// Sends a message and deletes it after a certian amount of time
// As well as logs to stdout if there's any error in sending
async fn temp_msg(ctx: &Context, duration: u64, result: SerenityResult<Message>) {
    match result {
        Ok(msg) => {
            let http = Arc::clone(&ctx.http);
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(duration)).await;
                let _ = msg.delete(http).await; 
            });
        }
        Err(why) => println!("Error sending message: {:?}", why),
    }
}

// Get the prefix for use with commands
async fn get_prefix(ctx: &Context, msg: &Message) -> String {
    let data = ctx.data.read().await;
    let database = data.get::<Database>().expect("Expected Database in TypeMap");
    
    match msg.guild_id {
        Some(gid) => return database.retrieve_str("guild_settings", "prefix", "id", &gid.0).await,
        None => return to_string("9!"),
    }
}


// =====================
// 
//     META MESSAGES
// 
// =====================



pub enum Meta {
    Ping,
}

#[allow(unreachable_patterns)]
pub async fn meta(ctx: &Context, msg: &Message, meta: Meta, args: &[&str]) {
    check_msg(msg.channel_id.send_message(ctx, |m| {
        m.reference_message(msg);
        m.embed(|e| {
            e.color(DEFAULT_COLOR);
            e.description(
                if !args.is_empty() {              
                    match meta {
                        _ => panic!("Specified embed with argurments when it shouldn't have been [meta]"),
                    }   
                } else {
                    match meta {
                        Meta::Ping => "Pong!",
                        _ => panic!("Specified embed was not provided with arguments [meta]"),
                    }.to_string()
                }
            );
            e
        });
        m
    }).await);
}

pub async fn help(ctx: &Context, msg: &Message) {
    check_msg(msg.channel_id.send_message(ctx, |m| m.embed(|e| {
        e.title("Commands");
        e.color(DEFAULT_COLOR);
        e.description("System
        - **Help** Show this message.
        - **Ping** Pong!
        - **Settings** Change how the bot behaves in this server.
        
        Moderation
        - **Stunlock** Manually stunlock a user.
        - **Streak** Modify a user's streak.");
        e
    })).await);
}



// ========================
// 
//     SETTINGS MESSAGES
// 
// ========================



pub enum Setting {
    CurrentPrefix,
    ChangedPrefix,
    AddedChannel,
    RemovedChannel,
    NoChannel,
    Malformed,
    EnabledGlobal,
    DisabledGlobal,
}

pub async fn setting(ctx: &Context, msg: &Message, setting: Setting, args: &[&str]) {
    check_msg(msg.channel_id.send_message(ctx, |m| {
        m.reference_message(msg);
        m.embed(|e| {
            e.color(SETTINGS_COLOR);
            e.description(
                if !args.is_empty() {              
                    match setting {
                        Setting::ChangedPrefix => format!("Changed the command prefix to: {}", args[0]),
                        Setting::CurrentPrefix => format!("The command prefix is: {}", args[0]),
                        Setting::AddedChannel => format!("Added `{}` to the channel whitelist", args[0]),
                        Setting::RemovedChannel => format!("Removed `{}` from the channel whitelist", args[0]),
                        _ => panic!("Specified embed with argurments when it shouldn't have been [settings]"),
                    }   
                } else {
                    match setting {
                        Setting::NoChannel => "Specified argument was not a channel.",
                        Setting::Malformed => "Specified argument was malformed.",
                        Setting::EnabledGlobal => "This guild will now use the global dataset.",
                        Setting::DisabledGlobal => "This guild will no longer use the global dataset",
                        _ => panic!("Specified embed was not provided with arguments [settings]"),
                    }.to_string()
                }
            );
            e
        });
        m
    }).await);
}

pub async fn list_settings(ctx: &Context, msg: &Message) {
    check_msg(msg.channel_id.send_message(ctx, |m| m.embed(|e| {
        e.title("Settings");
        e.color(SETTINGS_COLOR);
        e.description("- **Prefix** Change the command prefix.
        - **Whitelist** Add or remove channels to the whitelist.
        - **Global** Toggle use of the cross-server dataset.");
        e
    })).await);
}

pub async fn whitelisted(ctx: &Context, msg: &Message) {
    let data = ctx.data.read().await;
    let database = data.get::<Database>().expect("Expected database in TypeMap.");

    let channel_table = &format!("channels_{}", msg.guild_id.unwrap());
    let channel_ids = database.get_all_rows(channel_table, "id").await;
    let mut desc: String;
    if channel_ids.is_empty() {
        desc = "No channels have been whitelisted.\nDo /settings whitelist #channel".to_string();
    } else {
        desc = "Currently whitelisted channels:".to_string();
        for channel_id in channel_ids {
            // ChannelId(channel_id).name(&ctx.cache);
            desc.push_str(&format!("\n<#{}>", channel_id))
        }
    }

    check_msg(msg.channel_id.send_message(ctx, |m| m.embed(|e| {
        e.color(SETTINGS_COLOR);
        e.description(desc);
        e
    })).await);
}



// ========================
// 
//     GENERAL MESSAGES
// 
// ========================



pub async fn stunlock(ctx: &Context, msg: &FauxMessage, duration: u64, streak: u64) {
    temp_msg(ctx, 10, msg.channel_id.send_message(ctx, |m| {
        m.embed(|e| {
            e.color(ERROR_COLOR);
            e.title("Stunlocked");
            e.description(format!("<@{}> **was stunlocked for**: `{}`\n**Current streak**: `{}`",
              msg.author.id.0,
              seconds_to_string(duration),
              streak,
            ));
            e.thumbnail("https://i.imgur.com/IEZKNZE.png");
            e.field("\u{200B}", "[Why did I get stunlocked?](https://github.com/DontStarve72/Ninethousand-Eighty-Four#why-was-i-muted)", true);
            e
        });
        m
    }).await).await;
}

pub async fn manual_mute(ctx: &Context, msg: &Message, offender: &User) {
    check_msg(msg.channel_id.send_message(ctx, |m| {
        m.reference_message(msg);
        m.embed(|e| {
            e.color(DEFAULT_COLOR);
            e.description(format!("Manually stunlocked <@{}>.",
              offender.id.0,
            ));
            e
        });
        m
    }).await);
}

pub async fn manual_streak(ctx: &Context, msg: &Message, offender_id: &u64, streak: &u64) {
    check_msg(msg.channel_id.send_message(ctx, |m| {
        m.reference_message(msg);
        m.embed(|e| {
            e.color(DEFAULT_COLOR);
            e.description(format!("Set the streak of the user <@{}> to `{}`",
              offender_id,
              streak,
            ));
            e
        });
        m
    }).await);
}

// ========================
// 
//     DIRECT MESSAGES
// 
// ========================



pub async fn unmute(ctx: &Context, user: &User, guild_id: &u64) {
    let guild_name = GuildId(*guild_id).name(&ctx.cache).await;
    check_msg(user.direct_message(ctx, |m| {
        m.embed(|e| {
            e.color(DEFAULT_COLOR);
            match guild_name {
              Some(name) => e.description(format!("**Your stunlock in** `{}` **has ended**.", name)),
              None => e.description("Your stunlock has ended."),
            };
            e
        });
        m
    }).await);
}



// =======================
// 
//     ERORR MESSAGES
// 
// =======================



pub async fn no_user (ctx: &Context, msg: &Message) {
    let prefix = get_prefix(ctx, msg).await;

    check_msg(msg.channel_id.send_message(ctx, |m| {
        m.reference_message(msg);
        m.embed(|e| {
            e.color(ERROR_COLOR);
            e.description(format!("You must specify a user. See `{}help` for examples.", prefix));
            e
        });
        m
    }).await);
}

pub async fn no_int (ctx: &Context, msg: &Message) {
    let prefix = get_prefix(ctx, msg).await;
    
    check_msg(msg.channel_id.send_message(ctx, |m| {
        m.reference_message(msg);
        m.embed(|e| {
            e.color(ERROR_COLOR);
            e.description(format!("You must specify a number. See `{}help` for examples.", prefix));
            e
        });
        m
    }).await);
}

pub async fn streak_bad_size (ctx: &Context, msg: &Message) {
    check_msg(msg.channel_id.send_message(ctx, |m| {
        m.reference_message(msg);
        m.embed(|e| {
            e.color(ERROR_COLOR);
            e.description("Specified streak must be `at least 0` or `below 16`.");
            e
        });
        m
    }).await);
}