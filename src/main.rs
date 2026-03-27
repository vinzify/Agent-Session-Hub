mod app;
mod browser;
mod config;
mod formatting;
mod git;
mod paths;
mod provider;
mod session;
mod shell;

use anyhow::Result;

fn main() -> Result<()> {
    app::run()
}
