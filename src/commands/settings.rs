use serenity::{
    client::Context,
    framework::standard::{
        macros::command, 
        CommandResult,
        Args, ArgError,
    },
    model::{channel::{Message}, id::ChannelId},
};

use crate::{
    util::{
        misc::to_string, 
        embeds::{Setting, self}, 
        database::{INTEGER},
    }, 
    Database,
};

#[command]
#[only_in(guilds)]
#[required_permissions(MANAGE_GUILD)]
#[aliases(setting, options, option)]
// I would like to at some point learn exactly how decorators and macros work, I imagine this would be an appropriate use case?
async fn settings(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let setting = args.single::<String>();

    // If the message doesn't have a guild id attached to it, return the default id
    let guild_id: u64 = match msg.guild_id {
        Some(id) => id.0,
        None => 0,
    };

    match setting {
        Ok(s) => match s.to_lowercase().as_str() {
            // Prefix Setting
            "prefix" => {
                let data = ctx.data.read().await;
                let database = data.get::<Database>().expect("Expected Database in TypeMap");

                let arg1 = args.single::<String>();
                match arg1 {
                    // If a Prefix is specififed, change the prefix,
                    // unless the guild id is 0, in which case return the 0 (default) prefix
                    Ok(s) => {
                        if !database.row_exists("guild_settings", "id", &guild_id).await {
                            database.insert_row("guild_settings", &[&to_string(guild_id), "9!"]).await;
                        }
                        database.update_str("guild_settings", "prefix", &s, &guild_id).await;
                        embeds::setting(ctx, msg, Setting::ChangedPrefix,&[&s]).await;
                    }

                    // if no prefix is specified, say the current prefix for the server.
                    Err(_) => {
                        if !database.row_exists("guild_settings", "id", &guild_id).await {
                            database.insert_row("guild_settings", &[&to_string(guild_id), "9!"]).await;
                        }
                        let prefix = database.retrieve_str("guild_settings", "prefix","id", &guild_id).await;
                        embeds::setting(ctx, msg, Setting::CurrentPrefix, &[&prefix]).await;
                    }
                }
            },

            //Whitelist channels setting
            "whitelist" => {
                let arg1 = args.single::<ChannelId>();
                let data = ctx.data.read().await;
                let database = data.get::<Database>().expect("Expected Database in TypeMap");
                let channel_table = &format!("channels_{}", guild_id);
                
                match arg1 {
                    Ok(id) => {
                            match id.to_channel(&ctx.http).await {
                                Ok(channel) => {
                                    if let Some(c) = channel.guild() {
                                        if !database.table_exists(channel_table).await {
                                            database.create_table(channel_table, &["id"], &vec![INTEGER]).await;
                                        }
                                        
                                        // Channel is whitelisted
                                        if database.row_exists(channel_table, "id", &id.0).await {
                                            // Remove from whitelist
                                            database.delete_row(channel_table, "id", &id.0).await;
                                            embeds::setting(ctx, msg, Setting::RemovedChannel, &[&c.name]).await;
                                        }
                                        // Channel isn't whitelisted 
                                        else {
                                            // Add to whitelist
                                            database.insert_row(channel_table, &[&to_string(&id.0)]).await;
                                            embeds::setting(ctx, msg, Setting::AddedChannel, &[&c.name]).await;
                                        }
                                    } else {
                                        embeds::setting(ctx, msg, Setting::NoChannel, &[]).await;
                                    }
                                }
                                Err(_) => println!("Unable to get channel [settings whitelist]"),
                            }
                        }
                    Err(err) => {
                        match err {
                            ArgError::Eos => {
                                if !database.table_exists(channel_table).await {
                                    database.create_table(channel_table, &vec!["id"], &vec![INTEGER]).await;
                                }
        
                                // Print a list of all currently whitelist channels
                                embeds::whitelisted(ctx, msg).await;
                            }
                            _ => embeds::setting(ctx, msg, Setting::NoChannel, &[]).await,
                        }
                    }
                }
            },

            "global" => {
                let data = ctx.data.read().await;
                let database = data.get::<Database>().expect("Expected Database in TypeMap");

                let global = database.retrieve_bool("guild_settings", "global", "id", &guild_id).await;
                database.update_bool("guild_settings", "global", !global, &guild_id).await;

                if global {
                    embeds::setting(ctx, msg, Setting::DisabledGlobal, &[]).await;
                } else {
                    embeds::setting(ctx, msg, Setting::EnabledGlobal, &[]).await;
                }
            },


            // If a setting is specified which doesn't exist, declare as much, and print a list of available settings.
            _ => embeds::list_settings(ctx, msg).await,
        },

        // If no setting is specified, print a list of available settings.
        Err(err) => {
            match err {
                ArgError::Eos => embeds::list_settings(ctx, msg).await,
                _ => embeds::setting(ctx, msg, Setting::Malformed, &[]).await //Parse error
            }
        }
    };

    Ok(())
}
