use crate::{Event, EventList};

use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, ParseResult, Utc};

use ical::IcalParser;
use std::io::BufReader;

pub struct RemoteList {
    url: String,
}

impl RemoteList {
    pub fn new(s: &str) -> Self {
        Self { url: s.to_string() }
    }
}

fn parse_from_iso8601(s: &str) -> ParseResult<DateTime<Local>> {
    let naive = match NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S%Z") {
        Ok(d) => d,
        Err(_) => NaiveDate::parse_from_str(s, "%Y%m%d")?
            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Valid Time")),
    };

    Ok(DateTime::<Local>::from_utc(
        naive,
        FixedOffset::east_opt(0).expect("Valid Offset"),
    ))
}

fn unescape_ical(s: &str) -> String {
    s.replace("\\,", ",")
        .replace("\\\\", "\\")
        .replace("\\n", "\n")
        .replace("\\;", ";")
}

#[async_trait::async_trait]
impl EventList for RemoteList {
    async fn init(&mut self) {}

    async fn events(&self, after: DateTime<Local>) -> Vec<Event> {
        let resp = reqwest::get(&self.url).await.unwrap();

        let body = String::from_utf8(resp.bytes().await.unwrap().to_vec())
            .unwrap()
            .replace("\r\n\t", "");

        let ical = IcalParser::new(BufReader::new(stringreader::StringReader::new(&body)));

        ical
            .filter_map(|r| match r {
                Ok(cal) => Some(cal),
                Err(e) => {
                    log::error!("{:?}", e);
                    None
                }
            })
            .flat_map(|cal| {
                cal.events
                    .iter()
                    .map(|e| {
                        let mut out_event = Event::default();
                        for prop in &e.properties {
                            match prop.name.as_str() {
                                "DESCRIPTION" => {
                                    out_event.desc =
                                        unescape_ical(prop.value.as_ref().expect("No value"))
                                            .replace("\\n", "\n");
                                }
                                "SUMMARY" => {
                                    out_event.title =
                                        unescape_ical(prop.value.as_ref().expect("No value"))
                                            .replace("\\n", "\n");
                                }
                                "DTSTART" => {
                                    out_event.start =
                                        prop.value.as_ref().and_then(
                                            |s| match parse_from_iso8601(s) {
                                                Ok(d) => Some(d),
                                                Err(e) => {
                                                    log::error!("Date Parse error {e:?} {s}");
                                                    None
                                                }
                                            },
                                        );
                                }
                                "DTEND" => {
                                    out_event.end =
                                        prop.value.as_ref().and_then(
                                            |s| match parse_from_iso8601(s) {
                                                Ok(d) => Some(d),
                                                Err(e) => {
                                                    log::error!("Date Parse error {e:?} {s}");
                                                    None
                                                }
                                            },
                                        );
                                }
                                "DTSTAMP" => {}
                                "CLASS" => {}
                                "UID" => {}
                                "SEQUENCE" => {}
                                "LAST-MODIFIED" => {}
                                "LOCATION" => out_event.class = prop.value.clone(),
                                "RRULE" => {}
                                e => {
                                    panic!("Unimplemented property {e}");
                                }
                            }
                        }
                        out_event
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|e| DateTime::<Utc>::from(e.end.unwrap()) >= after)
            .collect::<Vec<_>>()
    }
}
