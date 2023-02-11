use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use teloxide::types::UserId;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub user_name: String,
    pub points: usize,
    pub tries: usize,
    pub tries_left: usize,
}
impl Record {
    pub fn new(name: String) -> Self {
        Self {
            user_name: name,
            points: 0,
            tries: 0,
            tries_left: 3,
        }
    }
    pub fn spin(&mut self, _points: SlotResult) {
        self.tries += 1;
        self.tries_left -= 1;
        self.points += usize::from(_points);
    }
}

impl Ord for Record {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.points.cmp(&other.points)
    }
}
impl PartialOrd for Record {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.points.cmp(&other.points))
    }
}

#[derive(Clone, Copy)]
pub enum SlotResult {
    Bars,
    Grapes,
    Lemons,
    Sevens,
    Nothing,
}

pub fn refresh_tries(_map: Arc<Mutex<BTreeMap<UserId, Record>>>) {
    _map.lock()
        .unwrap()
        .values_mut()
        .for_each(|x| x.tries_left = 3);
}

impl From<SlotResult> for String {
    fn from(val: SlotResult) -> Self {
        match val {
            SlotResult::Bars => String::from("бабки"),
            SlotResult::Grapes => String::from("бабулесы"),
            SlotResult::Lemons => String::from("бабло"),
            SlotResult::Sevens => String::from("деньги"),
            SlotResult::Nothing => String::from(""),
        }
    }
}

impl From<SlotResult> for usize {
    fn from(val: SlotResult) -> Self {
        match val {
            SlotResult::Bars => 1,
            SlotResult::Grapes => 2,
            SlotResult::Lemons => 3,
            SlotResult::Sevens => 5,
            SlotResult::Nothing => 0,
        }
    }
}
impl From<i32> for SlotResult {
    fn from(num: i32) -> Self {
        match num {
            1 => SlotResult::Bars,
            22 => SlotResult::Grapes,
            43 => SlotResult::Lemons,
            64 => SlotResult::Sevens,
            _ => SlotResult::Nothing,
        }
    }
}
