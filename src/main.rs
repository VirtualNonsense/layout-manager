use crate::app::App;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use std::io;

pub mod app;
pub mod event;
pub mod ui;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;

    let result = App::new().run(terminal).await;

    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();

    result
}
