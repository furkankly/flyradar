use std::io;

use clap::Parser;
use config::{FullConfig, TokenConfig};
use ops::{IoReqEvent, IoRespEvent, Ops};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tracing::error;
#[allow(unused_imports)]
use tracing_subscriber::prelude::*;

use crate::event::{Event, EventHandler};
use crate::handler::handle_key_events;
use crate::state::{RdrResult, State};
use crate::tui::Tui;

pub mod agent;
pub mod auth;
pub mod command;
pub mod config;
pub mod event;
pub mod fly_rust;
pub mod handler;
pub mod logs;
pub mod ops;
pub mod state;
pub mod transformations;
pub mod tui;
pub mod ui;
pub mod widgets;
pub mod wireguard;

#[cfg(debug_assertions)]
fn init_tracing() -> RdrResult<()> {
    tracing_subscriber::registry()
        // .with(tracing_subscriber::fmt::layer())
        // .with(tracing_subscriber::EnvFilter::new("hyper=debug"))
        .with(tui_logger::tracing_subscriber_layer())
        .init();
    tui_logger::init_logger(tracing::log::LevelFilter::Trace).unwrap();
    Ok(())
}

pub fn version() -> String {
    let commit_hash = option_env!("FLYRADAR_GIT_INFO").unwrap_or(env!("CARGO_PKG_VERSION"));
    let authors = clap::crate_authors!();

    format!(
        "\
{commit_hash}

Authors: {authors}"
    )
}

#[derive(Parser, Debug)]
#[command(version = version(), about = "Manage your Fly.io resources in style")]
struct Args {}

#[tokio::main]
async fn main() -> RdrResult<()> {
    #[cfg(debug_assertions)]
    init_tracing()?;
    Args::parse();
    color_eyre::install()?;

    if let Ok(access_token) = auth::read_access_token().await {
        let config = FullConfig {
            token_config: TokenConfig { access_token },
            wire_guard_state: None,
        };

        let (io_req_tx, mut io_req_rx) = tokio::sync::mpsc::channel::<IoReqEvent>(32);
        let (io_resp_tx, mut io_resp_rx) = tokio::sync::mpsc::channel::<IoRespEvent>(32);
        let mut state = State::default();
        state.init(io_req_tx);
        tokio::task::spawn(async move {
            let ops = Ops::new(config, io_resp_tx);
            while let Some(io_event) = io_req_rx.recv().await {
                let mut ops_clone = ops.clone();
                tokio::task::spawn(async move {
                    ops_clone.handle_io_req(io_event).await;
                });
            }
        });

        // Initialize the terminal user interface.
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;
        let events = EventHandler::new(250);
        let mut tui = Tui::new(terminal, events);
        tui.init()?;

        // Start the main loop.
        while state.running {
            // Render the user interface.
            tui.draw(&mut state)?;
            tokio::select! {
                Some(io_event) = io_resp_rx.recv() => {
                    state.handle_io_resp(io_event).await;
                }
                event = tui.events.next() => match event? {
                    Event::Tick => state.tick().await,
                    Event::Key(key_event) => {
                        let res = handle_key_events(key_event, &mut state).await;
                        if res.is_err() {
                            error!("Handle key event err: {:#?}", res);
                        }
                    }
                    Event::Mouse(_) => {}
                    Event::Resize(_, _) => {}
                }
            }
        }
        // Exit the user interface.
        tui.exit()?;
    }
    Ok(())
}
