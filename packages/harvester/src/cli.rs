//! Command-line interface for the harvester.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use crate::config::DEFAULT_MAX_RESPONSE_SIZE_MB;
use crate::error::{HarvesterError, Result};
use crate::http::create_client;
use crate::source::{self, BwbSource};
use crate::yaml::save_yaml;

/// RegelRecht Harvester - Download Dutch legislation from BWB and CVDR repositories.
#[derive(Parser)]
#[command(name = "regelrecht-harvester")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Download a law by BWB or CVDR ID and convert to YAML.
    Download {
        /// Law identifier: BWB ID (e.g., BWBR0018451) or CVDR ID (e.g., CVDR681386)
        law_id: String,

        /// Date in YYYY-MM-DD format; the version in force on that date is
        /// harvested (default: today)
        #[arg(short, long, conflicts_with_all = ["from", "to"])]
        date: Option<String>,

        /// Harvest one version per in-force date from this date on
        /// (YYYY-MM-DD, inclusive). BWB only. Each version is saved as
        /// `<valid_from>.yaml`. The version already in force on this date is
        /// included only if it starts on it; use `--date` for that one.
        #[arg(long, requires = "to")]
        from: Option<String>,

        /// Last in-force date to harvest with `--from` (YYYY-MM-DD, inclusive)
        #[arg(long, requires = "from")]
        to: Option<String>,

        /// Output directory (default: regulation/nl/)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Maximum response size in MB (default: 100)
        ///
        /// Increase this for exceptionally large laws like Wet op het financieel
        /// toezicht (52.6 MB). Most laws are under 5 MB.
        #[arg(long, default_value_t = DEFAULT_MAX_RESPONSE_SIZE_MB)]
        max_size: u64,
    },
}

/// Run the CLI.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Download {
            law_id,
            date,
            from,
            to,
            output,
            max_size,
        } => {
            let output = output.as_deref();
            validate_output_dir(output)?;
            match (from, to) {
                (Some(from), Some(to)) => {
                    download_period_command(&law_id, &from, &to, output, max_size).await
                }
                _ => download_command(&law_id, date.as_deref(), output, max_size).await,
            }
        }
    }
}

/// Build the appropriate `LawSource` for a CLI download.
///
/// Uses `detect_source` as the base, but applies the CLI-specific
/// `max_size_mb` override for BWB sources.
fn build_cli_source(law_id: &str, max_size_mb: u64) -> Result<Box<dyn source::LawSource>> {
    let law_source = source::detect_source(law_id)?;
    if law_source.source_type() == source::LawSourceType::Bwb {
        Ok(Box::new(BwbSource {
            max_size_mb: Some(max_size_mb),
        }))
    } else {
        Ok(law_source)
    }
}

/// Execute the download command.
async fn download_command(
    law_id: &str,
    date: Option<&str>,
    output: Option<&std::path::Path>,
    max_size_mb: u64,
) -> Result<()> {
    // Build source with CLI-specific max_size override (detect_source validates the ID)
    let law_source = build_cli_source(law_id, max_size_mb)?;

    // Use today if no date provided
    let effective_date = date
        .map(String::from)
        .unwrap_or_else(regelrecht_shared::dates::today_str);

    println!(
        "{} {} ({}) for date {}",
        style("Downloading").bold(),
        style(law_id).cyan(),
        law_source.name(),
        style(&effective_date).green()
    );
    println!();

    // Create progress spinner
    let pb = ProgressBar::new_spinner();
    #[allow(clippy::expect_used)] // Static template string that is guaranteed to be valid
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .expect("valid template"),
    );

    // Create HTTP client
    let client = create_client()?;

    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    pb.set_message(format!("Downloading from {}...", law_source.name()));
    let law = match law_source.download(&client, law_id, date).await {
        Ok(law) => law,
        Err(e) => {
            pb.finish_and_clear();
            return Err(e);
        }
    };

    pb.finish_and_clear();

    // BWB resolves the date to the consolidation in force on it; that
    // consolidation's start date is the version's valid_from.
    let valid_from = law
        .metadata
        .effective_date
        .clone()
        .unwrap_or(effective_date);
    let output_path = report_and_save(&law, &valid_from, output)?;

    println!();
    println!(
        "{} {}",
        style("Saved to:").green().bold(),
        output_path.display()
    );

    Ok(())
}

/// Execute the download command for a period: one version per in-force date.
async fn download_period_command(
    law_id: &str,
    from: &str,
    to: &str,
    output: Option<&std::path::Path>,
    max_size_mb: u64,
) -> Result<()> {
    let law_source = build_cli_source(law_id, max_size_mb)?;
    if law_source.source_type() != source::LawSourceType::Bwb {
        return Err(HarvesterError::InvalidLawId(format!(
            "{law_id}: --from/--to needs a BWB id; CVDR has no consolidations per in-force date"
        )));
    }

    println!(
        "{} {} (BWB), every in-force date from {} to {}",
        style("Downloading").bold(),
        style(law_id).cyan(),
        style(from).green(),
        style(to).green()
    );

    let client = create_client()?;
    // Each version is saved as soon as it is built, so a failure halfway
    // keeps the versions before it.
    let count =
        crate::harvester::download_law_versions(&client, law_id, from, to, max_size_mb, |law| {
            let valid_from = law
                .metadata
                .effective_date
                .clone()
                .unwrap_or_else(|| from.to_string());
            println!();
            println!("{}", style(&valid_from).green().bold());
            let output_path = report_and_save(&law, &valid_from, output)?;
            println!("  Saved to: {}", output_path.display());
            Ok(())
        })
        .await?;

    if count == 0 {
        println!("No in-force dates between {from} and {to}; nothing saved.");
        return Ok(());
    }

    println!();
    println!("{} {count} versions", style("Saved").green().bold());
    Ok(())
}

/// Print a law's summary and warnings, then save it as `<valid_from>.yaml`.
fn report_and_save(
    law: &crate::types::Law,
    valid_from: &str,
    output: Option<&std::path::Path>,
) -> Result<PathBuf> {
    println!("  Title: {}", style(&law.metadata.title).green());
    println!("  Type: {}", law.metadata.regulatory_layer.as_str());
    if let Some(creator) = &law.metadata.creator {
        println!("  Creator: {}", style(creator).cyan());
    }
    println!("  Articles: {}", law.articles.len());
    if !law.warnings.is_empty() {
        println!("  Warnings: {}", style(law.warnings.len()).yellow().bold());
        for warning in &law.warnings {
            println!("    - {warning}");
        }
    }
    save_yaml(law, valid_from, output)
}

/// Check that the output directory (if given) exists before downloading.
fn validate_output_dir(output: Option<&std::path::Path>) -> Result<()> {
    if let Some(output_dir) = output {
        if !output_dir.exists() {
            return Err(HarvesterError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Output directory does not exist: {}", output_dir.display()),
            )));
        }
        if !output_dir.is_dir() {
            return Err(HarvesterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Output path is not a directory: {}", output_dir.display()),
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_download_bwb() {
        let cli = Cli::parse_from(["regelrecht-harvester", "download", "BWBR0018451"]);

        let Commands::Download {
            law_id,
            date,
            from,
            to,
            output,
            max_size,
        } = cli.command;
        assert_eq!(law_id, "BWBR0018451");
        assert!(date.is_none());
        assert!(from.is_none());
        assert!(to.is_none());
        assert!(output.is_none());
        assert_eq!(max_size, 100); // Default 100 MB
    }

    #[test]
    fn test_cli_parse_download_cvdr() {
        let cli = Cli::parse_from(["regelrecht-harvester", "download", "CVDR681386"]);

        let Commands::Download { law_id, .. } = cli.command;
        assert_eq!(law_id, "CVDR681386");
    }

    #[test]
    fn test_cli_parse_download_with_date() {
        let cli = Cli::parse_from([
            "regelrecht-harvester",
            "download",
            "BWBR0018451",
            "--date",
            "2025-01-01",
        ]);

        let Commands::Download { law_id, date, .. } = cli.command;
        assert_eq!(law_id, "BWBR0018451");
        assert_eq!(date, Some("2025-01-01".to_string()));
    }

    #[test]
    fn test_cli_parse_download_with_max_size() {
        let cli = Cli::parse_from([
            "regelrecht-harvester",
            "download",
            "BWBR0018451",
            "--max-size",
            "200",
        ]);

        let Commands::Download { max_size, .. } = cli.command;
        assert_eq!(max_size, 200);
    }

    #[test]
    fn test_cli_parse_download_period() {
        let cli = Cli::parse_from([
            "regelrecht-harvester",
            "download",
            "BWBR0018472",
            "--from",
            "2024-01-01",
            "--to",
            "2026-12-31",
        ]);

        let Commands::Download { from, to, date, .. } = cli.command;
        assert_eq!(from.as_deref(), Some("2024-01-01"));
        assert_eq!(to.as_deref(), Some("2026-12-31"));
        assert!(date.is_none());
    }

    #[test]
    fn test_cli_period_rejects_half_or_mixed_dates() {
        let parse = |args: &[&str]| {
            let mut argv = vec!["regelrecht-harvester", "download", "BWBR0018472"];
            argv.extend_from_slice(args);
            Cli::try_parse_from(argv)
        };
        assert!(parse(&["--from", "2024-01-01"]).is_err());
        assert!(parse(&["--to", "2026-12-31"]).is_err());
        assert!(parse(&[
            "--date",
            "2025-01-01",
            "--from",
            "2024-01-01",
            "--to",
            "2026-12-31"
        ])
        .is_err());
    }

    #[test]
    fn test_build_cli_source_bwb() {
        let src = build_cli_source("BWBR0018451", 100).unwrap();
        assert_eq!(src.name(), "BWB");
    }

    #[test]
    fn test_build_cli_source_cvdr() {
        let src = build_cli_source("CVDR681386", 100).unwrap();
        assert_eq!(src.name(), "CVDR");
    }

    #[test]
    fn test_build_cli_source_invalid() {
        assert!(build_cli_source("INVALID", 100).is_err());
        assert!(build_cli_source("", 100).is_err());
    }
}
