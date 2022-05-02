mod util;
mod commands;
mod events;

use commands::{
    meta::*,
    settings::*,
    mute::*,
    set_streak::*,
};

use std::{
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering
        }
    },
    collections::{
        HashSet,
        HashMap
    },
    env,
    time::Duration
};

use serenity::{
    async_trait,
    client::bridge::gateway::ShardManager,
    framework::{
        standard::macros::{
                group,
                hook,
            },
        StandardFramework
    },
    http::Http,
    model::{
        event::{ResumedEvent, MessageUpdateEvent},
        gateway::Ready,
        channel::Message, id::GuildId,
    },
    prelude::*,
};

use tracing::{info, error};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use util::{
    database::{DatabaseTool},
    check::MuteInfo,
};

use crate::util::{
    database::{
        INTEGER,
        TEXT,
        BOOL,
    }, 
    misc::to_string, check,
};

#[group]
#[commands(
    ping, settings, help, mute_command, set_streak
)]
struct General;

struct Handler {
    is_loop_running: AtomicBool,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    async fn cache_ready(&self, ctx: Context, _: Vec<GuildId>) {
        let ctx = Arc::new(ctx);
        if !self.is_loop_running.load(Ordering::Relaxed) {
            let ctx1 = Arc::clone(&ctx);
            tokio::spawn(async move {
                loop {
                    check::check_loop(Arc::clone(&ctx1)).await;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });

            let ctx2 = Arc::clone(&ctx);
            tokio::spawn(async move {
                tokio::signal::ctrl_c()
                    .await
                    .expect("Could not register ctrl+c handler");
                println!("Shutting down...?");
        
                // Close the database
                let data = ctx2.data.read().await;
                let database = data.get::<Database>().expect("Expected Database in TypeMap");    
                database.pool.close().await;
                println!("Database SHOULD be closed");

                let shard_manager = data.get::<ShardManagerContainer>().expect("Expected Database in TypeMap");
                shard_manager.lock().await.shutdown_all().await;
            });

            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
        
        println!("Began looping through the mute cache");
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        events::on_message::message(ctx, msg).await;
    }

    async fn message_update(&self, ctx: Context, old: Option<Message>, new: Option<Message>, event: MessageUpdateEvent) {
        events::on_message::message_update(ctx, old, new, event).await;
    }
}

pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct Database;

impl TypeMapKey for Database {
    type Value = DatabaseTool;
}

struct MuteCache;

impl TypeMapKey for MuteCache {
    type Value = Arc<RwLock<HashMap<u64, HashMap<u64, MuteInfo>>>>;
}

struct Salt;

impl TypeMapKey for Salt {
    type Value = String;
}

#[hook]
async fn dynamic_prefix(ctx: &Context, msg: &Message) -> Option<String> {
    let guild_id = match msg.guild_id {
        Some(id) => id.0,
        None => 0,
    };

    let prefix: String;

    if guild_id != 0 {
        let data = ctx.data.read().await;
        let database = data.get::<Database>().expect("Expected Database in TypeMap");

        if !database.row_exists("guild_settings", "id", &guild_id).await {
            database.insert_row("guild_settings", &[&to_string(guild_id), "9!", "1"]).await;
        }
        prefix = database.retrieve_str("guild_settings", "prefix", "id", &guild_id).await;
    } else {
        prefix = "9!".to_string();
    }

    Some(prefix)
}

#[tokio::main]
async fn main() {
    // This will load the environment variables located at `./.env`, relative to
    // the CWD. See `./.env.example` for an example on how to structure this.
    dotenv::dotenv().expect("Failed to load .env file");

    // Initialize the logger to use environment variables.
    //
    // In this case, a good default is setting the environment variable
    // `RUST_LOG` to debug`.
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to start the logger");

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let http = Http::new_with_token(&token);

    // We will fetch your bot's owners and id
    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            owners.insert(info.owner.id);

            (owners, info.id)
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };
    
    // Create the framework
    let framework = StandardFramework::new()
        .configure(|c| {
            c
            .owners(owners)
            .on_mention(Some(bot_id))
            .dynamic_prefix(dynamic_prefix)
        })
        .group(&GENERAL_GROUP);

    let salt = env::var("SALT").expect("Expected a salt in the environment");
    
    let host = env::var("MYSQL_HOST").expect("Expected the database host in the environment");
    let username = env::var("MYSQL_USERNAME").expect("Expected the database username in the environment");
    let password = env::var("MYSQL_PASSWORD").expect("Expected the database password in the environment");
    let db = env::var("MYSQL_DB").expect("Expected the database name in the environment");

    let database = DatabaseTool {
        pool: sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(5)
        .connect_with(
            sqlx::mysql::MySqlConnectOptions::new()
                .host(&host)
                .username(&username)
                .password(&password)
                .database(&db),
        )
        .await
        .expect("Coundn't connect to database"),
    };

    let mut mute_map: HashMap<u64, HashMap<u64, MuteInfo>> = HashMap::new();
    
    if !database.table_exists("guild_settings").await {
        database.create_table("guild_settings", &vec!["id", "prefix", "global"], &vec![INTEGER, TEXT, BOOL]).await;
    }

    if !database.table_exists("global").await {
        database.create_table("global", &vec!["id"], &vec![TEXT]).await
    }

    
    for gid in database.get_all_rows("guild_settings", "id").await {
        let mut guild_data: HashMap<u64, MuteInfo> = HashMap::new();
        let stunlock_table = &format!("stunlocks_{}", gid);
        if database.table_exists(stunlock_table).await {
            for uid in database.get_all_rows(stunlock_table, "id").await {
                guild_data.insert(uid, MuteInfo {
                    streak: database.retrieve_int(stunlock_table, "streak", "id", &uid).await as u64,
                    streak_time: database.retrieve_int(stunlock_table, "streak_time", "id", &uid).await as u64,
                    mute_until: database.retrieve_int(stunlock_table, "mute_until", "id", &uid).await as u64, 
                });
            }
        }
        mute_map.insert(gid, guild_data);
    }

    let mute_cache = Arc::new(RwLock::new(mute_map));

    let mut client = Client::builder(&token)
        .framework(framework)
        .event_handler(Handler {
            is_loop_running: AtomicBool::new(false), 
        })
        .cache_settings(|s|
            s
            .max_messages(10000)
        )
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<Salt>(salt);
        data.insert::<Database>(database);
        data.insert::<MuteCache>(mute_cache);
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
    }

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
