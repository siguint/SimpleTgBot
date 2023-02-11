use crate::{utils::read_from_file, ADuel};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error};
use teloxide::{
    prelude::*,
    types::{CallbackQuery, MessageId, UserId},
};

pub enum Shoot {
    Loser(UserId),
    Draw,
    None,
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct DuelRecord {
    pub win: usize,
    pub lose: usize,
}

impl DuelRecord {
    fn new() -> Self {
        Self { win: 0, lose: 0 }
    }
}

impl Ord for DuelRecord {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.win.cmp(&other.win)
    }
}
impl PartialOrd for DuelRecord {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.win.cmp(&other.win))
    }
}

#[derive(Debug)]
pub struct OneDuel {
    pub time: i64,
    pub msg: MessageId,

    pub pushkin_name: String,
    pub pushkin: UserId,
    pub pushkin_throw: Option<i32>,

    pub dantes_name: String,
    pub dantes: UserId,
    pub dantes_throw: Option<i32>,
}
impl OneDuel {
    pub fn new(
        pushkin: UserId,
        dantes: UserId,
        pushkin_name: String,
        dantes_name: String,
        x: MessageId,
        t: i64,
    ) -> Self {
        Self {
            time: t,
            msg: x,
            pushkin_name,
            pushkin,
            pushkin_throw: None,
            dantes_name,
            dantes,
            dantes_throw: None,
        }
    }
    fn cannot_shoot(&self, x: UserId) -> bool {
        if x == self.pushkin && self.pushkin_throw.is_none() {
            false
        } else if x == self.dantes && self.dantes_throw.is_none() {
            false
        } else {
            true
        }
    }
    pub fn not_ready(&self) -> bool {
        self.pushkin_throw.is_none() || self.dantes_throw.is_none()
    }
    pub fn set_value(&mut self, id: UserId, value: i32) {
        if id == self.pushkin {
            self.pushkin_throw = Some(value);
        }
        if id == self.dantes {
            self.dantes_throw = Some(value);
        }
    }
    fn opponent_name(&self, id: UserId) -> (String, String) {
        if id == self.pushkin {
            (self.dantes_name.clone(), self.pushkin_name.clone())
        } else {
            (self.pushkin_name.clone(), self.dantes_name.clone())
        }
    }
    fn results(&self) -> (i32, i32) {
        (self.pushkin_throw.unwrap(), self.dantes_throw.unwrap())
    }
}

#[derive(Debug)]
pub struct Duel {
    records: HashMap<UserId, DuelRecord>,
    open_duels: HashMap<MessageId, OneDuel>,
    dices: Vec<MessageId>,
}

impl Duel {
    pub fn try_new(s: &str) -> Result<Self, Box<dyn Error>> {
        let records: HashMap<UserId, DuelRecord> = read_from_file(s)?;
        Ok(Self {
            records,
            open_duels: HashMap::new(),
            dices: vec![],
        })
    }
    pub fn save(&self, s: &str) {
        crate::to_writer(std::fs::File::create(s).unwrap(), &self.records).unwrap();
    }
    pub fn start_duel(
        &mut self,
        pushkin: UserId,
        dantes: UserId,
        user1: String,
        user2: String,
        x: MessageId,
        t: i64,
    ) {
        self.records.entry(pushkin).or_insert(DuelRecord::new());
        self.records.entry(dantes).or_insert(DuelRecord::new());

        self.open_duels
            .insert(x, OneDuel::new(pushkin, dantes, user1, user2, x, t));
    }

    pub fn shoot(&mut self, x: MessageId, id: UserId, value: i32) -> Shoot {
        match self.open_duels.get_mut(&x) {
            Some(y) => y.set_value(id, value),
            None => return Shoot::None,
        }

        if self.open_duels[&x].not_ready() {
            return Shoot::None;
        }

        let (pushkin, dantes) = self.open_duels[&x].results();
        match pushkin.cmp(&dantes) {
            std::cmp::Ordering::Less => Shoot::Loser(self.open_duels[&x].pushkin),
            std::cmp::Ordering::Equal => Shoot::Draw,
            std::cmp::Ordering::Greater => Shoot::Loser(self.open_duels[&x].dantes),
        }
    }
}

pub async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
    duel: ADuel,
) -> Result<(), teloxide::RequestError> {
    bot.answer_callback_query(&q.id).await?;
    let is_restricted = bot
        .get_chat_member(q.message.as_ref().unwrap().chat.id, q.from.id)
        .await?
        .is_restricted();
    if let Some(mm) = duel
        .lock()
        .unwrap()
        .open_duels
        .get(&q.message.as_ref().unwrap().id)
    {
        if mm.cannot_shoot(q.from.id) || dbg!(is_restricted) {
            dbg!(&mm);
            return Ok(());
        }
    }
    if let Some(_) = q.data {
        bot.answer_callback_query(q.id).await?;

        let tmpd = bot
            .send_dice(q.message.as_ref().unwrap().chat.id.clone())
            .emoji(teloxide::types::DiceEmoji::Dice)
            .await?;
        duel.lock().unwrap().dices.push(tmpd.id);

        let value = match tmpd.kind {
            teloxide::types::MessageKind::Dice(x) => x.dice.value,
            _ => 0,
        };
        let tmp = q.message.as_ref().unwrap();
        let text = format!(
            "{}\n{} выбрасывает {value} очков\n",
            tmp.text().unwrap(),
            q.from.mention().unwrap()
        );
        let user_id = q.from.id;
        let loser = duel.lock().unwrap().shoot(tmp.id, user_id, value);

        let cb = teloxide::types::InlineKeyboardButton::callback("шут", "шут");
        let kbd = teloxide::types::InlineKeyboardMarkup::new(vec![vec![cb]]);

        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
        if let Some(teloxide::types::Message {
            ref id, ref chat, ..
        }) = q.message
        {
            bot.edit_message_text(chat.id, *id, &text)
                .reply_markup(kbd)
                .await?;
        }
        match loser {
            Shoot::Loser(loser) => {
                let mut to_delete = vec![];
                std::mem::swap(&mut to_delete, &mut duel.lock().unwrap().dices);

                for x in to_delete {
                    bot.delete_message(tmp.chat.id, x).await?;
                }
                let (winner_name, loser_name) =
                    duel.lock().unwrap().open_duels[&tmp.id].opponent_name(loser);
                let text = format!(
                    "{}Побеждает {winner_name}🏆\n{loser_name} отправляется в бан☠️\n",
                    text
                );
                bot.edit_message_text(tmp.chat.id, tmp.id, text).await?;
                let time = duel.lock().unwrap().open_duels[&tmp.id].time;
                bot.restrict_chat_member(
                    tmp.chat.id,
                    loser,
                    teloxide::types::ChatPermissions::empty(),
                )
                .until_date(tmpd.date + chrono::Duration::minutes(time))
                .await?;
            }
            Shoot::Draw => {
                let text = format!(
                    "{}Ничья\n",
                    text
                );
                bot.edit_message_text(tmp.chat.id, tmp.id, text).await?;
            }
            Shoot::None => (),
        }
    }
    Ok(())
}
