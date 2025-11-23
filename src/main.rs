use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use display_tuner::display::{apply_display_config, enumerate_displays, DisplayConfig};

#[derive(Parser, Debug)]
#[command(name = "display-tuner", about = "Tune Windows display resolution and scaling", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List detected displays and their current settings
    List,
    /// Apply settings
    Set(SetArgs),
}

#[derive(clap::Args, Debug)]
struct SetArgs {
    /// Target display source id; omit applying to all or use --all
    #[arg(long)]
    id: Option<u32>,
    /// Apply to all displays (overrides --id)
    #[arg(long)]
    all: bool,
    /// Width in pixels (e.g. 1920)
    #[arg(long)]
    width: Option<u32>,
    /// Height in pixels (e.g. 1080)
    #[arg(long)]
    height: Option<u32>,
    /// Scaling percentage (100,125,150,175,...)
    #[arg(long)]
    scaling: Option<i32>,
}

fn main() -> Result<()> {
    //let subscriber = tracing_subscriber::fmt()
    //    .finish();
    //tracing::subscriber::set_global_default(subscriber)?;

    let cli = Cli::parse();
    
    match cli.command {
        Commands::List => {
            let displays = enumerate_displays()?;
            for d in &displays {
                println!("{d}");
            }
        }
        Commands::Set(args) => {
            let mut displays = enumerate_displays()?;

            if !args.all {
                if let Some(id) = args.id {
                    displays.retain(|d| d.source_id == id);
                } else {
                   return Err(anyhow!("No display source id specified"));
                }
            }

            if displays.is_empty() {
                return Err(anyhow!("No matching displays found"));
            }

            for disp in &displays {
                let target =
                    DisplayConfig {
                        width: args.width.unwrap_or(disp.width),
                        height: args.height.unwrap_or(disp.height),
                        scaling: args.scaling.unwrap_or(disp.scaling_current),
                    };
                println!("Applying to display {}: {target:?}", disp.source_id);
                apply_display_config(disp, &target)?;
            }
        }
    }

    Ok(())
}
