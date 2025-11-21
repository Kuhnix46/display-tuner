use anyhow::Result;
use tracing::info;

use crate::display::DisplayConfig;
mod display;

fn main() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let c1 = DisplayConfig {
        width: 1920,
        height: 1080,
        scaling: 100,
    };

    let c2 = DisplayConfig {
        width: 3840,
        height: 2160,
        scaling: 175,
    };

    let mut tuner = display::DisplayTuner::default();

    let displays = tuner.enumerate_displays()?;
    for disp in &displays {
        tuner.apply_display_config(disp, &c2)?;
    }

    Ok(())
}
