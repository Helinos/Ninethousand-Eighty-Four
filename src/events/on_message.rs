use serenity::{
    model::{
        channel::Message,
        event::MessageUpdateEvent,
        id::{ChannelId, MessageId},
        user::User,
    },
    prelude::*,
};

use crate::{
    util::misc::{to_string, self}, 
    check::mute,
    Database, Salt,
};

pub struct FauxMessage {
    pub content: String,
    pub author: User,
    pub id: MessageId,
    pub channel_id: ChannelId,
    pub guild_id: u64,
}

// When a message is sent
pub async fn message(ctx: Context, msg: Message) {
    let guild_id = match msg.guild_id {
        Some(id) => id.0,
        None => return, // If the message was sent in a DM don't do anything
    };

    check(
        ctx,
        &FauxMessage {
            content: msg.content,
            author: msg.author,
            id: msg.id,
            channel_id: msg.channel_id,
            guild_id: guild_id,
        },
    )
    .await;
}

// When a message is edited
pub async fn message_update(ctx: Context, old: Option<Message>, new: Option<Message>, event: MessageUpdateEvent) {
    let msg: FauxMessage;

    if let Some(m) = new {
        msg = FauxMessage {
            guild_id: match &m.guild_id {
                Some(id) => id.0,
                None => return, // If the message was sent in a DM don't do anything
            },
            content: m.content,
            author: m.author,
            id: m.id,
            channel_id: m.channel_id,
        }
    } else {
        // Fall back to the raw message update event
        if let (Some(c), Some(a), i, Some(g)) = (event.content, event.author, event.id, &event.guild_id) {
            msg = FauxMessage {
                guild_id: g.0,
                content: c,
                author: a,
                id: i,
                channel_id: event.channel_id,
            }
        } else {
            return;
        }
    }

    // If the message was updated and nothing was changed then don't do anything
    if let Some(o) = old {
        // Compare to the hashes of the strings because the hash function trims puntuation and whitespace
        if misc::hash(&o.content) == misc::hash(&msg.content) {
            return;
        }
    }

    check(ctx, &msg).await;
}

async fn check(ctx: Context, msg: &FauxMessage) {
    if msg.author.bot {
        return;
    }

    let data = ctx.data.read().await;
    let database = data.get::<Database>().expect("Expected Database in TypeMap");

    let guild_id = msg.guild_id;
    let channel_table = &format!("channels_{}", guild_id);
    
    let whitelisted: bool;
    if database.table_exists(channel_table).await {
        whitelisted = database.row_exists(channel_table, "id", &msg.channel_id.0).await;
    } else {
        whitelisted = false;
    }

    // If the server isn't using the global dataset, then salt the message with the salt along with the guild's and the channel's id
    let hash: u128;
    if database.retrieve_bool("guild_settings", "global", "id", &guild_id).await {
        // Global dataset enabled
        hash = misc::hash(&msg.content);
   } else {
        // Global dataset disabled
        if whitelisted {
            let salt = data.get::<Salt>().expect("Expected Salt in TypeMap");
            hash = misc::hash(&format!("{}{}{}{}", salt, msg.content, guild_id, msg.channel_id.0));
        } else {
            // If the channel isn't whitelisted there's no point in storing anything
            return;
        }
    }

    let infringing = database.row_exists("global", "id", &hash).await;
    if !infringing {
        database.insert_row("global", &[&to_string(hash)]).await
    }
    drop(database);
    
    if whitelisted && infringing{
        delete_message(&ctx, msg).await;
        mute(&ctx, msg).await;
    }
}

async fn delete_message(ctx: &Context, msg: &FauxMessage) {
    if let Ok(c) = msg.channel_id.to_channel(&ctx.http).await {
        if let Some(gc) = c.guild() {
            let _ = gc.delete_messages(&ctx.http, &[msg.id]).await;
        }
    }
}
