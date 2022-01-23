use serenity::{
    client::Context,
    framework::standard::{
        macros::command,
        CommandResult,
    },
    model::channel::Message,
};

use crate::util::embeds::{
    self,
    Meta,
};

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    embeds::meta(ctx, msg, Meta::Ping, &[]).await;
    Ok(())
}

#[command]
async fn help(ctx: &Context, msg: &Message) -> CommandResult {
    embeds::help(ctx, msg).await;
    Ok(())
}
