pub mod lists;

use std::{collections::HashMap, fmt::Display};

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub title: String,
    pub desc: String,
    pub end: Option<DateTime<Local>>,
    pub start: Option<DateTime<Local>>,
    pub class: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DayList {
    pub date: DateTime<Local>,
    pub events: Vec<Event>,
}

pub struct Days {
    list: Vec<DayList>,
}

impl Days {
    pub fn from_slice(list: &[Event]) -> Self {
        let mut preout = HashMap::<NaiveDate, Vec<Event>>::new();

        for e in list {
            if let Some(l) = preout.get_mut(&e.start.unwrap().with_timezone(&Local).date_naive()) {
                l.push(e.clone());
            } else {
                preout.insert(
                    e.start.unwrap().with_timezone(&Local).date_naive(),
                    vec![e.clone()],
                );
            }
        }

        let mut out: Vec<_> = preout
            .into_iter()
            .map(|(date, mut events)| {
                //println!("{:?} {:?}", date);

                events.sort_by_key(|e| e.start);

                DayList {
                    date: DateTime::<Local>::from_local(
                        date.and_hms_opt(0, 0, 0).expect("Valid time"),
                        *Local::now().offset(),
                    ),
                    events,
                }
            })
            .collect();

        out.sort_by_key(|day| day.date);

        Self { list: out }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.list)
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} {:?}:", self.title, self.end)?;
        writeln!(f, "{}", self.desc)?;

        Ok(())
    }
}

impl Default for Event {
    fn default() -> Self {
        Self {
            title: "Unknown".to_string(),
            desc: "Unknown".to_string(),
            end: None,
            start: None,
            class: None,
        }
    }
}

#[async_trait::async_trait]
pub trait EventList {
    async fn init(&mut self);

    async fn events(&self, after: DateTime<Local>) -> Vec<Event>;
}
