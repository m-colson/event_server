#![windows_subsystem = "windows"]

mod tray_icon;

use std::{convert::Infallible, net::SocketAddr, thread};

mod event_manager;
use event_manager::EventManager;

mod config;

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use tokio::sync::Mutex;

static MANAGER: Mutex<EventManager> = Mutex::const_new(EventManager::new());

static LOG_LIST: Mutex<Vec<String>> = Mutex::const_new(Vec::new());

static CONFIG_FILE: &str = ".manager_config.toml";

use log::info;

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn flush(&self) {}

    fn log(&self, record: &log::Record) {
        let rec_str = format!(
            "{} [{}] {:?}",
            record.module_path().unwrap(),
            record.level(),
            record
        );
        tokio::spawn(async move {
            LOG_LIST.lock().await.push(rec_str);
        });
    }
}

#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    log::set_max_level(log::LevelFilter::Info);
    match log::set_logger(&Logger) {
        Ok(_) => {}
        Err(e) => {eprintln!("Failed to start logging. Error: {e:?}")}
    };

    *MANAGER.lock().await = EventManager::from_config(CONFIG_FILE).await?;

    let (shutdown_send, shutdown_recv) = tokio::sync::oneshot::channel();

    thread::spawn(|| tray_icon::start_icon(shutdown_send));

    run_server(shutdown_recv).await?;

    Ok(())
}

/// Starts the server and awaits it
async fn run_server(shutdown_recv: tokio::sync::oneshot::Receiver<()>) -> Result<(), hyper::Error> {
    info!("Started server");
    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    let service = make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(serve_events)) });

    Server::bind(&addr)
        .serve(service)
        .with_graceful_shutdown(async move {
            shutdown_recv.await.unwrap();
        })
        .await?;

    info!("Exited server");
    Ok(())
}

async fn serve_events(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    info!("Request: {req:?}");

    let resp = match req.uri().path() {
        "/" => {
            let body = MANAGER
                .lock()
                .await
                .cached_json()
                .await
                .unwrap()
                .to_string();
            
            Response::builder()
                .header("Access-Control-Allow-Origin", "*")
                .body(Body::from(body))
                .unwrap()
        }
        "/log" => Response::builder()
            .header("Access-Control-Allow-Origin", "*")
            .body(Body::from(LOG_LIST.lock().await.join("\n")))
            .unwrap(),
        "/viewer" => Response::builder()
            .header("Access-Control-Allow-Origin", "*")
            .body(Body::from(include_str!("..\\..\\viewer\\index.html")))
            .unwrap(),
        _ => Response::builder()
            .status(404)
            .body(Body::from(""))
            .unwrap(),
    };

    Ok(resp)
}
