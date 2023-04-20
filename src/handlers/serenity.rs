use crate::{
    commands::{
        autopause::*, clear::*, leave::*, manage_sources::*, now_playing::*, pause::*, play::*,
        queue::*, remove::*, repeat::*, resume::*, seek::*, shuffle::*, skip::*, stop::*,
        summon::*, version::*, voteskip::*,
    },
    connection::{check_voice_connections, Connection},
    errors::ParrotError,
    guild::settings::{GuildSettings, GuildSettingsMap},
    handlers::track_end::update_queue_messages,
    sources::spotify::{Spotify, SPOTIFY},
    utils::create_response_text,
};
use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::{
        application::command::{Command, CommandOptionType},
        application::interaction::{
            application_command::ApplicationCommandInteraction, Interaction,
        },
        gateway::Ready,
        id::GuildId,
        prelude::{Activity, VoiceState},
    },
    prelude::Mentionable,
};

pub struct SerenityHandler;

#[async_trait]
impl EventHandler for SerenityHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("🦜 {} is connected!", ready.user.name);

        // sets parrot activity status message to /play
        let activity = Activity::listening("/play");
        ctx.set_activity(activity).await;

        // attempts to authenticate to spotify
        *SPOTIFY.lock().await = Spotify::auth().await;

        // creates the global application commands
        self.create_commands(&ctx).await;


        // loads serialized guild settings
        self.load_guilds_settings(&ctx, &ready).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::ApplicationCommand(mut command) = interaction else {
            return;
        };

        if let Err(err) = self.run_command(&ctx, &mut command).await {
            self.handle_error(&ctx, &mut command, err).await
        }
    }

    async fn voice_state_update(&self, ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
        // do nothing if this is a voice update event for a user, not a bot
        if new.user_id != ctx.cache.current_user_id() {
            return;
        }

        if new.channel_id.is_some() {
            return self.self_deafen(&ctx, new.guild_id, new).await;
        }

        let manager = songbird::get(&ctx).await.unwrap();
        let guild_id = new.guild_id.unwrap();

        if manager.get(guild_id).is_some() {
            manager.remove(guild_id).await.ok();
        }

        update_queue_messages(&ctx.http, &ctx.data, &[], guild_id).await;
    }
}

impl SerenityHandler {
    async fn create_commands(&self, ctx: &Context) -> Vec<Command> {
        Command::set_global_application_commands(&ctx.http, |commands| {
            commands
            .create_application_command(|command| {
                command
                    .name("autopause")
                    .description("Imposta se mettere in pausa la riproduzione alla fine di una traccia")
            })
                .create_application_command(|command| {
                    command
                        .name("clear")
                        .description("Svuota la coda")
                })
                .create_application_command(|command| {
                    command
                        .name("leave")
                        .description("Il bot lascia il canale a cui è connesso")
                })
                .create_application_command(|command| {
                    command
                        .name("managesources")
                        .description("Gestisci lo streaming dei contenuti da fonti diverse")
                })
                .create_application_command(|command| {
                    command
                        .name("np")
                        .description("Mostra informazioni della traccia in riproduzione")
                })
                .create_application_command(|command| {
                    command
                        .name("pause")
                        .description("Mette in pausa la traccia corrente")
                })
                .create_application_command(|command| {
                    command
                        .name("play")
                        .description("Aggiunge una traccia alla coda")
                        .create_option(|option| {
                                option
                                    .name("query")
                                    .description("Il contenuto da riprodurre")
                                    .kind(CommandOptionType::String)
                                    .required(true)
                        })
                })
                .create_application_command(|command| {
                    command
                        .name("superplay")
                        .description("Aggiungi una traccia alla coda in maniera speciale")
                        .create_option(|option| {
                            option
                                .name("next")
                                .description("Aggiunge una traccia da riprodurre successivamente")
                                .kind(CommandOptionType::SubCommand)
                                .create_sub_option(|option| {
                                    option
                                        .name("query")
                                        .description("Il contenuto da riprodurre")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                        })
                        .create_option(|option| {
                            option
                                .name("jump")
                                .description("Riproduce una traccia instantaneamente, saltando quella attuale")
                                .kind(CommandOptionType::SubCommand)
                                .create_sub_option(|option| {
                                    option.name("query")
                                    .description("Il contenuto da riprodurre")
                                    .kind(CommandOptionType::String)
                                    .required(true)
                                })
                        })
                        .create_option(|option| {
                            option
                                .name("all")
                                .description("Aggiunge tutte le tracce se lo URL si riferisce a un video e una playlist")
                                .kind(CommandOptionType::SubCommand)
                                .create_sub_option(|option| {
                                    option
                                        .name("query")
                                        .description("Il contenuto da riprodurre")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                        })
                        .create_option(|option| {
                            option
                                .name("reverse")
                                .description("Aggiunge una playlist alla coda ma in ordine inverso")
                                .kind(CommandOptionType::SubCommand)
                                .create_sub_option(|option| {
                                    option
                                        .name("query")
                                        .description("The media to play")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                        })
                        .create_option(|option| {
                            option
                                .name("shuffle")
                                .description("Aggiunge una playlist alla coda ma in ordine casuale")
                                .kind(CommandOptionType::SubCommand)
                                .create_sub_option(|option| {
                                    option
                                        .name("query")
                                        .description("Il contenuto da riprodurre")
                                        .kind(CommandOptionType::String)
                                        .required(true)
                                })
                        })
                })
                .create_application_command(|command| {
                    command
                        .name("queue")
                        .description("Mostra la coda")
                })
                .create_application_command(|command| {
                    command
                        .name("remove")
                        .description("Rimuove una traccia dalla coda")
                        .create_option(|option| {
                            option
                                .name("index")
                                .description("Posizione della traccia nella coda (1 è la prossima traccia ad essere riprodotta)")
                                .kind(CommandOptionType::Integer)
                                .required(true)
                                .min_int_value(1)
                        })
                        .create_option(|option| {
                            option
                                .name("until")
                                .description("Posizione della traccia dell'intervallo superiore per rimuovere un intervallo di tracce")
                                .kind(CommandOptionType::Integer)
                                .required(false)
                                .min_int_value(1)
                        })
                })
                .create_application_command(|command| {
                    command
                        .name("repeat")
                        .description("Attiva/Disattiva la ripetizione della traccia corrente")
                })
                .create_application_command(|command| {
                    command
                        .name("resume")
                        .description("Riprende la traccia attuale")
                })
                .create_application_command(|command| {
                    command
                        .name("seek")
                        .description("Cerca la traccia corrente al minutaggio inserito")
                        .create_option(|option| {
                            option
                                .name("timestamp")
                                .description("Minutaggio nel formato HH:MM:SS")
                                .kind(CommandOptionType::String)
                                .required(true)
                        })
                })
                .create_application_command(|command| {
                    command.name("shuffle").description("Mischia la coda")
                })
                .create_application_command(|command| {
                    command.name("skip").description("Salta la traccia corrente")
                    .create_option(|option| {
                        option
                            .name("to")
                            .description("Indice del brano al quale skippare")
                            .kind(CommandOptionType::Integer)
                            .required(false)
                            .min_int_value(1)
                    })
                })
                .create_application_command(|command| {
                    command
                        .name("stop")
                        .description("Ferma il bot e svuota la coda")
                })
                .create_application_command(|command| {
                    command
                        .name("summon")
                        .description("Richiama il bot nel proprio canale vocale")
                })
                .create_application_command(|command| {
                    command
                        .name("version")
                        .description("Visualizza la versione corrente")
                })
                .create_application_command(|command| {
                    command.name("voteskip").description("Avvia una votazione per saltare il brano corrente")
                })
        })
        .await
        .expect("failed to create command")
    }

    async fn load_guilds_settings(&self, ctx: &Context, ready: &Ready) {
        println!("[INFO] Loading guilds' settings");
        let mut data = ctx.data.write().await;
        for guild in &ready.guilds {
            println!("[DEBUG] Loading guild settings for {:?}", guild);
            let settings = data.get_mut::<GuildSettingsMap>().unwrap();

            let guild_settings = settings
                .entry(guild.id)
                .or_insert_with(|| GuildSettings::new(guild.id));

            if let Err(err) = guild_settings.load_if_exists() {
                println!(
                    "[ERROR] Failed to load guild {} settings due to {}",
                    guild.id, err
                );
            }
        }
    }

    async fn run_command(
        &self,
        ctx: &Context,
        command: &mut ApplicationCommandInteraction,
    ) -> Result<(), ParrotError> {
        let command_name = command.data.name.as_str();

        let guild_id = command.guild_id.unwrap();
        let guild = ctx.cache.guild(guild_id).unwrap();

        // get songbird voice client
        let manager = songbird::get(ctx).await.unwrap();

        // parrot might have been disconnected manually
        if let Some(call) = manager.get(guild.id) {
            let mut handler = call.lock().await;
            if handler.current_connection().is_none() {
                handler.leave().await.unwrap();
            }
        }

        // fetch the user and the bot's user IDs
        let user_id = command.user.id;
        let bot_id = ctx.cache.current_user_id();

        match command_name {
            "autopause" | "clear" | "leave" | "pause" | "remove" | "repeat" | "resume" | "seek"
            | "shuffle" | "skip" | "stop" | "voteskip" => {
                match check_voice_connections(&guild, &user_id, &bot_id) {
                    Connection::User(_) | Connection::Neither => Err(ParrotError::NotConnected),
                    Connection::Bot(bot_channel_id) => {
                        Err(ParrotError::AuthorDisconnected(bot_channel_id.mention()))
                    }
                    Connection::Separate(_, _) => Err(ParrotError::WrongVoiceChannel),
                    _ => Ok(()),
                }
            }
            "play" | "superplay" | "summon" => {
                match check_voice_connections(&guild, &user_id, &bot_id) {
                    Connection::User(_) => Ok(()),
                    Connection::Bot(_) if command_name == "summon" => {
                        Err(ParrotError::AuthorNotFound)
                    }
                    Connection::Bot(_) if command_name != "summon" => {
                        Err(ParrotError::WrongVoiceChannel)
                    }
                    Connection::Separate(bot_channel_id, _) => {
                        Err(ParrotError::AlreadyConnected(bot_channel_id.mention()))
                    }
                    Connection::Neither => Err(ParrotError::AuthorNotFound),
                    _ => Ok(()),
                }
            }
            "np" | "queue" => match check_voice_connections(&guild, &user_id, &bot_id) {
                Connection::User(_) | Connection::Neither => Err(ParrotError::NotConnected),
                _ => Ok(()),
            },
            _ => Ok(()),
        }?;

        match command_name {
            "autopause" => autopause(ctx, command).await,
            "clear" => clear(ctx, command).await,
            "leave" => leave(ctx, command).await,
            "managesources" => allow(ctx, command).await,
            "np" => now_playing(ctx, command).await,
            "pause" => pause(ctx, command).await,
            "play" | "superplay" => play(ctx, command).await,
            "queue" => queue(ctx, command).await,
            "remove" => remove(ctx, command).await,
            "repeat" => repeat(ctx, command).await,
            "resume" => resume(ctx, command).await,
            "seek" => seek(ctx, command).await,
            "shuffle" => shuffle(ctx, command).await,
            "skip" => skip(ctx, command).await,
            "stop" => stop(ctx, command).await,
            "summon" => summon(ctx, command, true).await,
            "version" => version(ctx, command).await,
            "voteskip" => voteskip(ctx, command).await,
            _ => unreachable!(),
        }
    }

    async fn self_deafen(&self, ctx: &Context, guild: Option<GuildId>, new: VoiceState) {
        let Ok(user) = ctx.http.get_current_user().await else {
            return;
        };

        if user.id == new.user_id && !new.deaf {
            guild
                .unwrap()
                .edit_member(&ctx.http, new.user_id, |n| n.deafen(true))
                .await
                .unwrap();
        }
    }

    async fn handle_error(
        &self,
        ctx: &Context,
        interaction: &mut ApplicationCommandInteraction,
        err: ParrotError,
    ) {
        create_response_text(&ctx.http, interaction, &format!("{err}"))
            .await
            .expect("failed to create response");
    }
}
