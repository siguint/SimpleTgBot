use casino::{Record, SlotResult};
use dptree::filter;
use duel::callback_handler;
use duel::Duel;
use inline_python::python;
use rand::Rng;
use sedregex::find_and_replace;
use serde_json::to_writer;
use std::io::prelude::*;
use std::{
    collections::BTreeMap,
    error::Error,
    sync::{Arc, Mutex},
};
use teloxide::{
    prelude::*,
    types::{Dice, DiceEmoji::SlotMachine, MessageKind, Update, UserId},
    utils::command::BotCommands,
};
use utils::read_from_file;

pub mod casino;
pub mod duel;
pub mod filters;
pub mod utils;

type Casino = Arc<Mutex<BTreeMap<UserId, Record>>>;
type MarkovModel = Arc<Mutex<inline_python::Context>>;
type ADuel = Arc<Mutex<Duel>>;
// type RandomIter = Arc<Mutex<FnOnce>>;


#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
struct MyConfig {
    bot_token: String,

    bot_maintainer_id: u64,
    maintainer_useraname: String,
    casino_file: String,
    messages_file: String,
    duel_file: String,

    test_chat: i64,

}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    log::info!("Starting dispatching features bot...");
    let cfg: MyConfig = confy::load("ayabot", None)?;
    let bot = Bot::new(cfg.bot_token);

    let casino: Casino = Arc::new(Mutex::new(read_from_file(&cfg.casino_file)?));
    let messages = Arc::new(Mutex::new(
        std::fs::File::options().append(true).open(&cfg.casino_file)?,
    ));
    let duel = Duel::try_new(&cfg.duel_file).unwrap();
    let aduel = Arc::new(Mutex::new(duel));

    let parameters = ConfigParameters {
        bot_maintainer: UserId(cfg.bot_maintainer_id),
        maintainer_username: Some(cfg.maintainer_useraname),
    };

    let c = Arc::new(Mutex::new(inline_python::Context::new()));
    let (context_copy, casino_copy) = (c.clone(), casino.clone());
    println!("lol");

    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400));
        loop {
            interval.tick().await;
            let my_copy = casino_copy.clone();
            to_writer(std::fs::File::create(&cfg.casino_file).unwrap(), &my_copy).unwrap();
            casino::refresh_tries(my_copy);
            let learn_text = std::fs::read_to_string(&cfg.messages_file).unwrap();
            context_copy.lock().unwrap().run(python! {
                import markovify
                text_model = markovify.NewlineText('learn_text)
            });
        }
    });

    let handler = dptree::entry()
        .branch(Update::filter_message()
        .filter(move |msg: Message| ChatId(cfg.test_chat) == msg.chat.id)
        .branch(
            dptree::entry()
                // Filter commands: the next handlers will receive a parsed `SimpleCommand`.
                .filter_command::<SimpleCommand>()
                // If a command parsing fails, this handler will not be executed.
                .endpoint(simple_commands_handler),
        )
        .branch(
            // Filter a maintainer by a used ID.
            filter(|msg: Message, cfg: ConfigParameters| {
                msg.from()
                    .map(|user| user.id == cfg.bot_maintainer)
                    .unwrap_or_default()
            })
            .filter_command::<MaintainerCommands>()
            .endpoint(
                |cmd: MaintainerCommands,
                 casino: Casino| async move {
                    match cmd {
                        MaintainerCommands::Refresh => {
                            casino::refresh_tries(casino.clone());
                            Ok(())
                        }
                    }
                },
            ),
        )
        .branch(
            Message::filter_dice().endpoint(|msg: Message, _dice: Dice, bot: Bot| async move {
                match msg.kind {
                    MessageKind::Dice(_x) /* if x.dice.emoji == teloxide::types::DiceEmoji::SlotMachine */ => {
                        bot.delete_message(msg.chat.id, msg.id).await?;
                    }
                    _ => (),
                }
                Ok(())
            }),
            )
        .branch(
            dptree::entry().filter(filters::sed_request).endpoint(
                |bot: Bot, msg: Message| async move {
                    let text = find_and_replace(msg.reply_to_message().unwrap().text().unwrap_or_default(), [msg.text().unwrap_or_default()]).unwrap();
                    bot.send_message(msg.chat.id, text).reply_to_message_id(msg.reply_to_message().unwrap().id).await?;
                    Ok(())
                }
                )
            )
        .branch(
            dptree::entry().endpoint(
                |bot: Bot, msg: Message, messages: Arc<Mutex<std::fs::File>>, context: MarkovModel| async move {
                    match msg.kind {
                        MessageKind::Common(_) => {
                            let secret_number = rand::thread_rng().gen_range(1..10);
                            if secret_number == 1 {
                                let string = msg.text().unwrap_or_default();
                                context.lock().unwrap().run(python! {
                                    my_string = None
                                    for word in 'string.split():
                                        try:
                                            my_string = text_model.make_sentence_with_start(word, strict=False)
                                        except:
                                            pass
                                    if my_string == None:
                                        my_string = ""
                                });
                                let text = context.lock().unwrap().get::<String>("my_string");
                                bot.send_message(msg.chat.id, text).await?;
                            }
                            messages.lock().unwrap().write_all(
                                ("\n".to_string() + msg.text().unwrap_or_default()).as_bytes(),
                                )?;
                        }
                        _ => (),
                    }
                    Ok(())
                },
            ),
        ))
        .branch(Update::filter_callback_query().endpoint(callback_handler));
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            parameters,
            casino.clone(),
            messages.clone(),
            c.clone(),
            aduel.clone()
        ])
        // If no handler succeeded to handle an update, this closure will be called.
        .default_handler(|upd| async move {
            log::warn!("Unhandled update: {:?}", upd);
        })
        // If the dispatcher fails for some reason, execute this handler.
        .error_handler(LoggingErrorHandler::with_custom_text(
            "An error has occurred in the dispatcher",
        ))
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    handle.abort();
    to_writer(std::fs::File::create("casinoo.txt").unwrap(), &casino)?;
    aduel.clone().lock().unwrap().save("duel.txt");
    Ok(())
}

#[derive(Clone)]
struct ConfigParameters {
    bot_maintainer: UserId,
    maintainer_username: Option<String>,
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Simple commands")]
enum SimpleCommand {
    #[command(description = "shows this message.")]
    Help,
    #[command(description = "shows maintainer info.")]
    Maintainer,
    #[command(description = "slot")]
    Slot,
    #[command(description = "top")]
    Top,
    #[command(description = "generate")]
    Markov { string: String },
    #[command(description = "duel")]
    Duel { time: i64 },
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Maintainer commands")]
enum MaintainerCommands {
    #[command(parse_with = "split", description = "Обновить попытки")]
    Refresh,
}

async fn simple_commands_handler(
    msg: Message,
    bot: Bot,
    cmd: SimpleCommand,
    cfg: ConfigParameters,
    me: teloxide::types::Me,
    casino: Casino,
    context: MarkovModel,
    duel: ADuel,
) -> Result<(), teloxide::RequestError> {
    if let SimpleCommand::Slot = cmd {
        if casino
            .lock()
            .unwrap()
            .get(&msg.from().unwrap().id)
            .map(|x| x.tries_left == 0)
            .unwrap_or(false)
        {
            bot.delete_message(msg.chat.id, msg.id).await?;
            return Ok(());
        }
        let tmp = bot.send_dice(msg.chat.id).emoji(SlotMachine).await?;
        let slot_result = SlotResult::from(match tmp.kind {
            MessageKind::Dice(x) => x.dice.value,
            _ => 0,
        });
        let user_struct = msg.from().unwrap();
        let user_id = user_struct.id;
        casino
            .lock()
            .unwrap()
            .entry(user_id)
            .or_insert(Record::new(user_struct.full_name()))
            .spin(slot_result);
        let tmpx = casino.lock().unwrap().get(&user_id).unwrap().clone();
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
        bot.send_message(
            msg.chat.id,
            format!(
                "{} \nчисло спинов {} число очков {}",
                String::from(slot_result),
                tmpx.tries,
                tmpx.points
            ),
        )
        .reply_to_message_id(msg.id)
        .await?;

        bot.delete_message(tmp.chat.id, tmp.id).await?;
        return Ok(());
    }
    if let SimpleCommand::Duel { time } = cmd {
        let time = match time {
            16.. => 15,
            2..=15 => time,
            i64::MIN..=1 => 2,
        };
        let _pushkin = msg.from().unwrap().to_owned();
        let dantes = msg.reply_to_message();
        if dantes.is_none() {
            return Ok(());
        }
        let _dantes = dantes.unwrap().from().unwrap().to_owned();

        let cb = teloxide::types::InlineKeyboardButton::callback("шут", "шут");
        let kbd = teloxide::types::InlineKeyboardMarkup::new(vec![vec![cb]]);
        let mymsg = bot
            .send_message(
                msg.chat.id,
                format!(
                    "⚔️Дуэль между {} и {}⚔️\n⏱Ставка - {time} минут мута🙊\nБросайте кубики🎲\n",
                    _pushkin.mention().unwrap(),
                    _dantes.mention().unwrap()
                ),
            )
            .reply_markup(kbd)
            .await?;
        duel.lock().unwrap().start_duel(
            _pushkin.id,
            _dantes.id,
            _pushkin.full_name(),
            _dantes.full_name(),
            mymsg.id,
            time,
        );
        return Ok(());
    }
    let text = match cmd {
        SimpleCommand::Help => {
            if msg.from().unwrap().id == cfg.bot_maintainer {
                format!(
                    "{}\n\n{}",
                    SimpleCommand::descriptions(),
                    MaintainerCommands::descriptions()
                )
            } else if msg.chat.is_group() || msg.chat.is_supergroup() {
                SimpleCommand::descriptions()
                    .username_from_me(&me)
                    .to_string()
            } else {
                SimpleCommand::descriptions().to_string()
            }
        }
        SimpleCommand::Maintainer => {
            if msg.from().unwrap().id == cfg.bot_maintainer {
                "Maintainer is you!".into()
            } else if let Some(username) = cfg.maintainer_username {
                format!("Maintainer is @{username}")
            } else {
                format!("Maintainer ID is {}", cfg.bot_maintainer)
            }
        }
        SimpleCommand::Markov { string } => {
            if !string.is_empty() {
                context.lock().unwrap().run(python! {
                    my_string = None
                    for word in 'string.split():
                        try:
                            my_string = text_model.make_sentence_with_start(word, strict=False)
                        except:
                            pass
                    if my_string == None:
                        my_string = ""
                });
            } else if let Some(x) = msg.reply_to_message() {
                let x = x.text().unwrap_or("lol").split(' ').last().unwrap();
                context.lock().unwrap().run(python! {
                    my_string = None
                    for word in 'x.split():
                        try:
                            my_string = text_model.make_sentence_with_start(word, strict=False)
                        except:
                            pass
                    if my_string == None:
                        my_string = ""
                });
            } else {
                context.lock().unwrap().run(python! {
                    my_string = text_model.make_sentence()
                    if my_string == None:
                        my_string = ""
                });
            }
            context.lock().unwrap().get::<String>("my_string")
        }
        SimpleCommand::Top => {
            let mut my_vec: Vec<Record> = casino.lock().unwrap().clone().into_values().collect();
            my_vec.sort();
            my_vec.reverse();
            format!("{:?}", my_vec)
        }
        _ => "lol".to_string(),
    };

    bot.send_message(msg.chat.id, text).await?;

    Ok(())
}
