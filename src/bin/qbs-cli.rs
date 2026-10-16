use anyhow::{Context, Result};
use qbs::{
    browser::{BrowserId, BrowserSnapshot},
    config::{self, SwitchMode},
    switching,
};

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let snapshot = BrowserSnapshot::discover();
    if args.is_empty() || args.iter().any(|a| a == "--status") {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }
    let target = args
        .windows(2)
        .find(|w| w[0] == "--browser")
        .and_then(|w| BrowserId::parse(&w[1]))
        .context("use --browser chrome|edge")?;
    let config = config::load();
    let mode = if args.iter().any(|a| a == "--supported") {
        SwitchMode::Supported
    } else {
        config.switch_mode
    };
    let result = switching::switch(
        &snapshot,
        target,
        mode,
        config.experimental_warning_acknowledged,
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
