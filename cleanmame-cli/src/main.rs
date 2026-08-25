use std::{path::PathBuf, process::Command};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use cleanmame_core::{
    Region,
    operations::{
        delete::delete_roms,
        filter::{FilterOptions, filter_roms},
        r#move::move_roms,
        query::{
            find_by_name, load_metadata, load_metadata_from_str, scan_rom_folder_with_entries,
        },
        report::generate_report,
    },
};
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(
    name = "cleanmame",
    version,
    about = "Manage MAME ROM folders using mame.xml and catver.ini"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Scan(MetadataArgs),
    Query(QueryArgs),
    Filter(FilterArgs),
    Move(MoveArgs),
    Delete(DeleteArgs),
    Report(MetadataArgs),
}

#[derive(Parser, Debug)]
struct MetadataArgs {
    #[arg(long)]
    rom_folder: PathBuf,
    #[arg(long, conflicts_with = "mame_executable")]
    mame_xml: Option<PathBuf>,
    #[arg(long)]
    mame_executable: Option<PathBuf>,
    #[arg(long)]
    catver: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Parser, Debug)]
struct QueryArgs {
    #[command(flatten)]
    metadata: MetadataOnlyArgs,
    #[arg(long)]
    name: String,
    #[arg(long)]
    json: bool,
}

#[derive(Parser, Debug)]
struct FilterArgs {
    #[command(flatten)]
    metadata: MetadataArgs,
    #[arg(long)]
    genre: Option<String>,
    #[arg(long, value_enum)]
    region: Option<RegionArg>,
    #[arg(long)]
    include_mature: bool,
    #[arg(long)]
    include_mechanical: bool,
    #[arg(long)]
    include_prototype: bool,
}

#[derive(Parser, Debug)]
struct MoveArgs {
    #[command(flatten)]
    filter: FilterArgs,
    #[arg(long)]
    target_folder: PathBuf,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Parser, Debug)]
struct DeleteArgs {
    #[command(flatten)]
    filter: FilterArgs,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Parser, Debug)]
struct MetadataOnlyArgs {
    #[arg(long, conflicts_with = "mame_executable")]
    mame_xml: Option<PathBuf>,
    #[arg(long)]
    mame_executable: Option<PathBuf>,
    #[arg(long)]
    catver: Option<PathBuf>,
}

#[derive(Clone, Debug, ValueEnum)]
enum RegionArg {
    Usa,
    Japan,
    Europe,
    World,
    Asia,
    Unknown,
}

impl From<RegionArg> for Region {
    fn from(value: RegionArg) -> Self {
        match value {
            RegionArg::Usa => Region::Usa,
            RegionArg::Japan => Region::Japan,
            RegionArg::Europe => Region::Europe,
            RegionArg::World => Region::World,
            RegionArg::Asia => Region::Asia,
            RegionArg::Unknown => Region::Unknown,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan(args) => {
            let roms = scan_from_args(&args)?;
            output(args.json, &roms, || {
                format!(
                    "Found {} metadata entries, {} available ROM files",
                    roms.len(),
                    roms.iter().filter(|rom| rom.rom_path.is_some()).count()
                )
            })?;
        }
        Commands::Query(args) => {
            let roms = metadata_entries(&args.metadata)?;
            let rom = find_by_name(&roms, &args.name)
                .with_context(|| format!("ROM '{}' was not found", args.name))?;
            output(args.json, rom, || format_rom(rom))?;
        }
        Commands::Filter(args) => {
            let json = args.metadata.json;
            let roms = filter_from_args(&args)?;
            output(json, &roms, || format_names(&roms))?;
        }
        Commands::Move(args) => {
            let json = args.filter.metadata.json;
            let roms = filter_from_args(&args.filter)?;
            let moved = move_roms(&roms, args.target_folder, args.dry_run)?;
            output(json, &moved, || format!("Moved {} ROM(s)", moved.len()))?;
        }
        Commands::Delete(args) => {
            let json = args.filter.metadata.json;
            let roms = filter_from_args(&args.filter)?;
            let deleted = delete_roms(&roms, args.dry_run)?;
            output(json, &deleted, || {
                format!("Deleted {} ROM(s)", deleted.len())
            })?;
        }
        Commands::Report(args) => {
            let roms = scan_from_args(&args)?;
            let report = generate_report(&roms);
            output(args.json, &report, || {
                format!(
                    "Total: {}\nAvailable: {}\nGenres: {}",
                    report.total,
                    report.available,
                    report.by_genre.len()
                )
            })?;
        }
    }

    Ok(())
}

fn scan_from_args(args: &MetadataArgs) -> Result<Vec<cleanmame_core::RomEntry>> {
    let roms = metadata_entries(&MetadataOnlyArgs {
        mame_xml: args.mame_xml.clone(),
        mame_executable: args.mame_executable.clone(),
        catver: args.catver.clone(),
    })?;
    scan_rom_folder_with_entries(&args.rom_folder, roms).map_err(Into::into)
}

fn filter_from_args(args: &FilterArgs) -> Result<Vec<cleanmame_core::RomEntry>> {
    let roms = scan_from_args(&args.metadata)?;
    Ok(filter_roms(
        &roms,
        &FilterOptions {
            genre_contains: args.genre.clone(),
            region: args.region.clone().map(Into::into),
            include_mature: args.include_mature,
            include_mechanical: args.include_mechanical,
            include_prototype: args.include_prototype,
            only_available: true,
        },
    ))
}

fn metadata_entries(args: &MetadataOnlyArgs) -> Result<Vec<cleanmame_core::RomEntry>> {
    if let Some(mame_xml) = &args.mame_xml {
        return load_metadata(mame_xml, args.catver.as_deref()).map_err(Into::into);
    }

    let executable = args
        .mame_executable
        .as_ref()
        .context("--mame-xml or --mame-executable is required")?;
    let output = Command::new(executable)
        .arg("-listxml")
        .output()
        .with_context(|| format!("failed to run {} -listxml", executable.display()))?;

    if !output.status.success() {
        bail!(
            "{} -listxml exited with {}",
            executable.display(),
            output.status
        );
    }

    let xml = String::from_utf8(output.stdout).context("MAME -listxml output was not UTF-8")?;
    load_metadata_from_str(&xml, args.catver.as_deref()).map_err(Into::into)
}

fn output<T: Serialize>(json: bool, value: &T, text: impl FnOnce() -> String) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", text());
    }
    Ok(())
}

fn format_rom(rom: &cleanmame_core::RomEntry) -> String {
    format!(
        "{} - {}",
        rom.name,
        rom.description.as_deref().unwrap_or("no description")
    )
}

fn format_names(roms: &[cleanmame_core::RomEntry]) -> String {
    roms.iter()
        .map(|rom| rom.name.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
