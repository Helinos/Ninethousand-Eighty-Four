use serenity::{
    framework::standard::{
        macros::command,
        CommandResult,
        Args
    },
    model::{
        channel::Message,
        id::UserId
    }, 
    client::Context, 
};

use crate::{
    events::on_message::FauxMessage,
    util::{
        embeds,
        check::mute,
    }
};


#[command]
#[required_permissions(MANAGE_MESSAGES)]
#[aliases(mute, stunlock)]
async fn mute_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg = args.single::<UserId>();

    match arg {
        Ok(offender_id) => {
            match offender_id.to_user(&ctx.http).await {
                Ok(offender) => {
                    embeds::manual_mute(ctx, msg, &offender).await;

                    // This is scuffed
                    // Neither the content nor the id need to passed to the mute function
                    // So the content is blank and the message id is syphoned from the muter's message
                    let fmsg: FauxMessage = FauxMessage {
                        content: "".to_string(),
                        author: offender,
                        id: msg.id,
                        channel_id: msg.channel_id,
                        guild_id: msg.guild_id.unwrap().0, // TODO: Check if the bot will try to mute from DMs
                    };

                    mute(ctx, &fmsg).await;
                }

                Err(why) => {
                    println!("Unable to retrieve user [Mute Command] Why: {:?}", why);
                }
            }
        }

        Err(_) => {
            embeds::no_user(ctx, msg).await;
        }
    }
    
    Ok(())
}
