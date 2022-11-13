use crate::{Event, EventList};

use google_calendar::Client;

use std::sync::Arc;
use tokio::sync::Mutex;

use tokio::sync::mpsc::{self, Sender};

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::collections::HashMap;
use std::{convert::Infallible, net::SocketAddr};

#[derive(Clone, Debug)]
struct ClientInfo {
    pub code: String,
    pub state: String,
}

lazy_static::lazy_static! {
    static ref INFO_SENDER: Arc<Mutex<Option<Sender<ClientInfo>>>> = Arc::new(Mutex::const_new(None));
}

async fn handle(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    match req.uri().path() {
        "/auth/" => {
            log::info!("{:?}", req);
            let mut vars = HashMap::new();

            for l in req.uri().query().unwrap().split('&') {
                let parts = l.split('=').collect::<Vec<_>>();

                assert_eq!(parts.len(), 2);

                vars.insert(parts[0], parts[1]);
            }

            let code = vars.get("code").unwrap().to_string();
            let state = vars.get("state").unwrap().to_string();

            INFO_SENDER
                .lock()
                .await
                .as_ref()
                .unwrap()
                .send(ClientInfo { code, state })
                .await
                .unwrap();

            log::info!("Responding");

            Ok(Response::new("".into()))
        }
        _ => Ok(Response::builder()
            .status(404)
            .body(Body::from(""))
            .unwrap()),
    }
}

pub struct GoogleList {
    calendar: Client,
}

impl GoogleList {
    pub async fn from_file(name: &str) -> Self {
        let data = String::from_utf8(std::fs::read(name).unwrap()).unwrap();

        let google_calender = match data.split('\n').collect::<Vec<_>>()[..] {
            [token, refresh_token] => Client::new(
                include_str!("../../tokens/client_id.token").to_string(),
                include_str!("../../tokens/client_secret.token").to_string(),
                "http://localhost:3000/auth/".to_string(),
                token.to_string(),
                refresh_token.to_string(),
            ),
            _ => unreachable!(),
        };

        match google_calender.refresh_access_token().await {
            Err(e) => {
                log::error!("Error Refreshing token: {e}");
                log::error!("Starting new token generation");
                return Self::new(name).await;
            }
            Ok(at) => {
                if at.access_token != data.split('\n').collect::<Vec<_>>()[0] {
                    std::fs::write(
                        name,
                        format!("{}\n{}", at.access_token, at.refresh_token,),
                    )
                    .unwrap();
                }
            }
        };

        Self {
            calendar: google_calender,
        }
    }

    pub async fn new(out_name: &str) -> Self {
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

        let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle)) });

        let (sender, mut reciever) = mpsc::channel::<ClientInfo>(16);

        *INFO_SENDER.lock().await = Some(sender);

        let mut google_calender = Client::new(
            include_str!("../../tokens/client_id.token").to_string(),
            include_str!("../../tokens/client_secret.token").to_string(),
            "http://localhost:3000/auth/".to_string(),
            "".to_string(),
            "".to_string(),
        );

        let user_consent_url = google_calender
            .user_consent_url(&["https://www.googleapis.com/auth/calendar.events".to_string()]);

        let server = Server::bind(&addr).serve(make_svc);

        match open::that(&user_consent_url) {
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to open consent url \"{user_consent_url}\" for reason {e:?}")
            }
        };

        let server_proc = tokio::spawn(async { server.await });

        let client_info = reciever.recv().await;

        server_proc.abort();

        log::info!("Sever finisished");

        if let Some(c) = client_info {
            let token = google_calender
                .get_access_token(&c.code, &c.state)
                .await
                .unwrap();

            std::fs::write(
                out_name,
                format!(
                    "{}\n{}",
                    token.access_token,
                    token.refresh_token,
                ),
            )
            .unwrap();
        } else {
            panic!("No client info!");
        }

        Self {
            calendar: google_calender,
        }
    }
}

#[async_trait::async_trait]
impl EventList for GoogleList {
    async fn init(&mut self) {}

    async fn events(&self, after: chrono::DateTime<chrono::Local>) -> Vec<Event> {
        self
            .calendar
            .events()
            .list_all(
                "primary",
                "",
                0,
                google_calendar::types::OrderBy::Noop,
                &[],
                "",
                &[],
                false,
                true,
                false,
                "",
                &after.to_rfc3339(),
                "",
                "",
            )
            .await
            .unwrap()
            .into_iter()
            .map(|e| Event {
                title: e.summary.clone(),
                desc: e.description.clone(),
                start: e.start.as_ref().and_then(|m| m.date_time).map(|a| a.into()),
                end: e.end.as_ref().and_then(|m| m.date_time).map(|a| a.into()),
                class: None,
            })
            .collect::<Vec<_>>()
    }
}
