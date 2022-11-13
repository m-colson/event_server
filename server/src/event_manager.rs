use chrono::{DateTime, Local, Utc};

use crate::{
    config::ManagerConfig,
};

use event_list::{lists::{google::GoogleList, remoteical::RemoteList}, Days, Event, EventList};

use std::error::Error;

pub struct EventManager {
    streams: Vec<Box<dyn EventList + Sync + Send>>,
    cached: Option<String>,
    cached_time: DateTime<Utc>,
}

impl EventManager {
    /// Creates an `EventManager` with no sources
    pub const fn new() -> Self {
        Self {
            streams: Vec::new(),
            cached: None,
            cached_time: DateTime::<Utc>::MIN_UTC,
        }
    }

    /// Adds a new source to this manager
    pub fn add<T: EventList + Sync + Send + 'static>(&mut self, list: T) {
        self.streams.push(Box::new(list));
    }

    /// Creates a new EventManager from the config `file`
    /// Returns an error if the file read fails or the config file couldn't be parsed
    pub async fn from_config(file: &str) -> Result<Self, Box<dyn Error>> {
        let mut out = EventManager::new();

        let config = toml::from_slice::<ManagerConfig>(&std::fs::read(file)?)?;

        for r in config.remotes {
            out.add(RemoteList::new(&r));
        }

        if config.google_calendar.auth {
            out.add(GoogleList::new(&config.google_calendar.token_file).await);
        } else if config.google_calendar.enabled {
            out.add(GoogleList::from_file(&config.google_calendar.token_file).await);
        }

        Ok(out)
    }

    /// Queries all the sources and turns it into json string that is a list of `Days`
    async fn events_json(&self) -> Result<String, serde_json::Error> {
        Days::from_slice(&self.events(Utc::now().with_timezone(&Local)).await).to_json()
    }

    /// Does the same as `events_json` but caches the result for an hour
    pub async fn cached_json(&mut self) -> Result<&str, serde_json::Error> {
        if Utc::now().signed_duration_since(self.cached_time) > chrono::Duration::hours(1) {
            self.cached = Some(self.events_json().await?);
            self.cached_time = Utc::now();
        }

        if self.cached.is_none() {
            self.cached = Some(self.events_json().await?);
            self.cached_time = Utc::now();
        }

        Ok(self.cached.as_ref().expect("Something is cached"))
    }
}

#[async_trait::async_trait]
impl EventList for EventManager {
    async fn init(&mut self) {
        futures::future::join_all(self.streams.iter_mut().map(|s| s.init())).await;
    }

    async fn events(&self, after: chrono::DateTime<chrono::Local>) -> Vec<Event> {
        futures::future::join_all(self.streams.iter().map(|s| s.events(after)))
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    }
}
