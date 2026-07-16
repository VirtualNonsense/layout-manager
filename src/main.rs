//! Entry point for the layout-manager demo application.
//!
//! Initialises the Ratatui terminal, enables mouse capture, runs [`App`], then
//! restores the terminal unconditionally on exit — even if the app returns an
//! error.

use crate::{app::App, log::init_logging};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use std::io;

pub mod app;
pub mod event;
mod log;
pub mod ui;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let _log_guard = init_logging()?;

    let terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;

    let result = App::new().run(terminal).await;

    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();

    result
}
