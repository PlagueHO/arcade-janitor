mod cli;

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
    ffi::OsString,
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use anyhow::{Context, Result, bail};
use arcadejanitor_core::{
    Region, RomEntry,
    metadata::{
        MetadataSourceTarget, clear_managed_cache, managed_cache_paths, plan_clear_managed_cache,
        refresh_catver, refresh_mame_xml, resolve_catver, resolve_mame_xml,
    },
    operations::{
        delete::delete_roms,
        filter::{FilterOptions, filter_roms},
        r#move::move_roms,
        query::{find_by_name, load_metadata, scan_rom_folder_with_entries_and_progress},
    },
    utils::filesystem::list_rom_files,
};
use chrono::{DateTime, Local};
use clap::{
    ColorChoice, CommandFactory, FromArgMatches,
    builder::styling::{AnsiColor, Styles},
};
use cli::{
    AuditLevel, CatalogCommands, CategoryCommands, Cli, ColorMode, Commands, McpCommands,
    McpInstallArgs, McpStartArgs, McpSystem, OrderingArgs, OutputFormat, PresentationOptions,
    RegionArg, RomCommands, RomStatus, SelectorArgs, SortField, SourceCommands, SourceOptions,
    SourceTarget,
};
use serde::Serialize;
use serde_json::json;

const LOGO: &str = concat!(
    "\n\x1b[38;5;39m",
    r"   _____                            .___            ____.             .__  __",
    "\n\x1b[38;5;39m",
    r"  /  _  \_______   ____ _____     __| _/____       |    |____    ____ |__|/  |_  ___________",
    "\n\x1b[38;5;45m",
    r" /  /_\  \_  __ \_/ ___\\__  \   / __ |/ __ \      |    \__  \  /    \|  \   __\/  _ \_  __ \",
    "\n\x1b[38;5;51m",
    r"/    |    \  | \/\  \___ / __ \_/ /_/ \  ___/  /\__|    |/ __ \|   |  \  ||  | (  <_> )  | \/",
    "\n\x1b[38;5;87m",
    r"\____|__  /__|    \___  >____  /\____ |\___  > \________(____  /___|  /__||__|  \____/|__|",
    "\n\x1b[38;5;87m",
    r"        \/            \/     \/      \/    \/                \/     \/",
    "\n\x1b[0m"
);

fn main() -> Result<()> {
    let cli = Cli::from_arg_matches(&cli_command().get_matches())?;
    let source = cli.source.apply_environment();
    validate_source_options(&source)?;
    init_diagnostics(&cli.presentation);

    match cli.command {
        Commands::Rom(command) => {
            run_rom(command.command, &source, &cli.presentation)?;
        }
        Commands::Catalog(command) => {
            run_catalog(command.command, &source, &cli.presentation)?;
        }
        Commands::Category(command) => {
            run_category(command.command, &source, &cli.presentation)?;
        }
        Commands::Source(command) => {
            run_source(command.command, &source, &cli.presentation)?;
        }
        Commands::Mcp(command) => {
            run_mcp(command.command, &source)?;
        }
        Commands::Completions(args) => {
            clap_complete::generate(
                args.shell,
                &mut cli_command(),
                "arcadejanitor",
                &mut std::io::stdout(),
            );
        }
    }

    Ok(())
}

fn cli_command() -> clap::Command {
    let styles = Styles::styled()
        .header(AnsiColor::BrightCyan.on_default().bold())
        .usage(AnsiColor::BrightBlue.on_default().bold())
        .literal(AnsiColor::BrightGreen.on_default().bold());
    let arguments = std::env::args().collect::<Vec<_>>();
    let color_argument = arguments.iter().enumerate().find_map(|(index, argument)| {
        argument
            .strip_prefix("--color=")
            .map(ToOwned::to_owned)
            .or_else(|| {
                (argument == "--color")
                    .then(|| arguments.get(index + 1).cloned())
                    .flatten()
            })
    });
    let color =
        if std::env::var_os("NO_COLOR").is_some() || color_argument.as_deref() == Some("never") {
            ColorChoice::Never
        } else if color_argument.as_deref() == Some("always") {
            ColorChoice::Always
        } else {
            ColorChoice::Auto
        };
    let mut command = Cli::command().color(color).styles(styles);
    if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        command = command.before_help(LOGO);
    }
    command
}

fn init_diagnostics(presentation: &PresentationOptions) {
    let filter = if presentation.quiet {
        "off"
    } else {
        match presentation.verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };
    let ansi = match presentation.color {
        ColorMode::Auto => {
            std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
        }
        ColorMode::Always => std::env::var_os("NO_COLOR").is_none(),
        ColorMode::Never => false,
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(ansi)
        .with_writer(std::io::stderr)
        .try_init();
}

fn validate_source_options(source: &SourceOptions) -> Result<()> {
    if source.mame_xml.is_some() && source.mame_executable.is_some() {
        bail!(
            "--mame-xml conflicts with --mame-executable, including values supplied by environment variables"
        );
    }
    Ok(())
}

fn run_mcp(command: McpCommands, source: &SourceOptions) -> Result<()> {
    match command {
        McpCommands::Start(args) => start_mcp_server(args, source),
        McpCommands::Install(args) => install_mcp_server(args, source),
    }
}

fn start_mcp_server(args: McpStartArgs, source: &SourceOptions) -> Result<()> {
    let executable = mcp_server_executable()?;
    let mut command = Command::new(&executable);
    command.arg("--rom-folder").arg(args.rom_dir);
    command.args(mcp_source_arguments(source));

    let status = command
        .status()
        .with_context(|| format!("failed to start MCP server at {}", executable.display()))?;
    if !status.success() {
        bail!("MCP server exited with status {status}");
    }
    Ok(())
}

fn mcp_server_executable() -> Result<PathBuf> {
    let executable =
        std::env::current_exe().context("could not determine the CLI executable path")?;
    let file_name = format!("arcadejanitor-mcp{}", std::env::consts::EXE_SUFFIX);
    Ok(executable.with_file_name(file_name))
}

fn install_mcp_server(args: McpInstallArgs, source: &SourceOptions) -> Result<()> {
    let mcp_executable = mcp_server_executable()?;
    let server_arguments = mcp_server_arguments(&args.rom_dir, source);
    let command = mcp_install_command(args.system, &mcp_executable, &server_arguments)?;
    let status = Command::new(&command.program)
        .args(&command.arguments)
        .status()
        .with_context(|| {
            format!(
                "failed to run {} while installing the MCP server; ensure it is installed and available on PATH",
                command.program.display(),
            )
        })?;
    if !status.success() {
        bail!(
            "{} exited with status {status} while installing the MCP server",
            command.program.display()
        );
    }
    Ok(())
}

fn mcp_server_arguments(rom_dir: &Path, source: &SourceOptions) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("--transport"),
        OsString::from("stdio"),
        OsString::from("--rom-folder"),
        rom_dir.as_os_str().to_os_string(),
    ];
    arguments.extend(mcp_source_arguments(source));
    arguments
}

fn mcp_source_arguments(source: &SourceOptions) -> Vec<OsString> {
    let mut arguments = Vec::new();
    if let Some(path) = &source.mame_xml {
        arguments.extend([
            OsString::from("--mame-xml"),
            path.as_os_str().to_os_string(),
        ]);
    }
    if let Some(path) = &source.mame_executable {
        arguments.extend([
            OsString::from("--mame-executable"),
            path.as_os_str().to_os_string(),
        ]);
    }
    if let Some(path) = &source.catver {
        arguments.extend([OsString::from("--catver"), path.as_os_str().to_os_string()]);
    }
    arguments
}

struct InstallCommand {
    program: OsString,
    arguments: Vec<OsString>,
}

fn mcp_install_command(
    system: McpSystem,
    mcp_executable: &Path,
    server_arguments: &[OsString],
) -> Result<InstallCommand> {
    let server_command = mcp_executable
        .to_str()
        .context("MCP server executable path is not valid Unicode")?;
    match system {
        McpSystem::VsCode | McpSystem::VsCodeInsiders => {
            let arguments = server_arguments
                .iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            let configuration = json!({
                "name": "arcadejanitor",
                "type": "stdio",
                "command": server_command,
                "args": arguments,
            });
            Ok(InstallCommand {
                program: OsString::from(match system {
                    McpSystem::VsCode => "code",
                    McpSystem::VsCodeInsiders => "code-insiders",
                    _ => unreachable!("stable and Insiders VS Code targets only"),
                }),
                arguments: vec![
                    OsString::from("--add-mcp"),
                    OsString::from(serde_json::to_string(&configuration)?),
                ],
            })
        }
        McpSystem::CopilotCli => {
            let mut arguments = vec![
                OsString::from("mcp"),
                OsString::from("add"),
                OsString::from("--transport"),
                OsString::from("stdio"),
                OsString::from("arcadejanitor"),
                OsString::from("--"),
                mcp_executable.as_os_str().to_os_string(),
            ];
            arguments.extend_from_slice(server_arguments);
            Ok(InstallCommand {
                program: OsString::from("copilot"),
                arguments,
            })
        }
        McpSystem::ClaudeCode => {
            let mut arguments = vec![
                OsString::from("mcp"),
                OsString::from("add"),
                OsString::from("--scope"),
                OsString::from("user"),
                OsString::from("--transport"),
                OsString::from("stdio"),
                OsString::from("arcadejanitor"),
                OsString::from("--"),
                mcp_executable.as_os_str().to_os_string(),
            ];
            arguments.extend_from_slice(server_arguments);
            Ok(InstallCommand {
                program: OsString::from("claude"),
                arguments,
            })
        }
    }
}

fn run_rom(
    command: RomCommands,
    source: &SourceOptions,
    presentation: &PresentationOptions,
) -> Result<()> {
    match command {
        RomCommands::List(args) => {
            let mut roms = scan_roms(&args.rom_dir, source, presentation)?;
            roms.retain(|rom| matches_status(rom, args.status));
            let include_non_runnable = matches!(args.status, RomStatus::Unmatched | RomStatus::All);
            roms = select_roms(roms, &args.selectors, include_non_runnable)?;
            sort_roms(&mut roms, &args.ordering);
            render_roms(&roms, presentation)?;
        }
        RomCommands::Show(args) => {
            let roms = scan_roms(&args.rom_dir, source, presentation)?;
            let rom = find_by_name(&roms, &args.name)
                .with_context(|| format!("ROM '{}' was not found", args.name))?;
            render_roms(std::slice::from_ref(rom), presentation)?;
        }
        RomCommands::Stats(args) => {
            let roms = select_roms(
                scan_roms(&args.rom_dir, source, presentation)?,
                &args.selectors,
                true,
            )?;
            let stats = CollectionStats::from_roms(&roms, args.show_missing, args.show_unmatched);
            render_stats(&stats, presentation)?;
        }
        RomCommands::Audit(args) => {
            let roms = scan_roms(&args.rom_dir, source, presentation)?;
            let findings = audit_collection(&args.rom_dir, &roms)?;
            let failed = findings.iter().any(|finding| finding.level >= args.fail_on);
            render_audit(&findings, presentation)?;
            if failed {
                std::process::exit(2);
            }
        }
        RomCommands::Move(args) => {
            require_mutation_selection(&args.selectors)?;
            let selected = select_roms(
                scan_roms(&args.rom_dir, source, presentation)?,
                &args.selectors,
                true,
            )?
            .into_iter()
            .filter(|rom| rom.rom_path.is_some())
            .collect::<Vec<_>>();
            require_matches(&selected)?;
            let mut plan = plan_move(&selected, &args.destination)?;
            let mut failures = 0;
            if args.execute {
                for (rom, operation) in selected.iter().zip(&mut plan) {
                    match move_roms(std::slice::from_ref(rom), &args.destination, false) {
                        Ok(_) => operation.state = "executed".to_string(),
                        Err(error) => {
                            operation.state = format!("error: {error}");
                            failures += 1;
                        }
                    }
                }
            }
            render_operation_rows(&plan, presentation)?;
            if failures > 0 {
                bail!("{failures} move operation(s) failed");
            }
        }
        RomCommands::Delete(args) => {
            require_mutation_selection(&args.selectors)?;
            let selected = select_roms(
                scan_roms(&args.rom_dir, source, presentation)?,
                &args.selectors,
                true,
            )?
            .into_iter()
            .filter(|rom| rom.rom_path.is_some())
            .collect::<Vec<_>>();
            require_matches(&selected)?;
            let mut plan = plan_delete(&selected)?;
            let mut failures = 0;
            if args.execute {
                for (rom, operation) in selected.iter().zip(&mut plan) {
                    match delete_roms(std::slice::from_ref(rom), false) {
                        Ok(_) => operation.state = "executed".to_string(),
                        Err(error) => {
                            operation.state = format!("error: {error}");
                            failures += 1;
                        }
                    }
                }
            }
            render_operation_rows(&plan, presentation)?;
            if failures > 0 {
                bail!("{failures} delete operation(s) failed");
            }
        }
    }
    Ok(())
}

fn run_catalog(
    command: CatalogCommands,
    source: &SourceOptions,
    presentation: &PresentationOptions,
) -> Result<()> {
    match command {
        CatalogCommands::List(args) => {
            let mut roms =
                select_roms(load_catalog(source, presentation)?, &args.selectors, false)?;
            sort_roms(&mut roms, &args.ordering);
            render_roms(&roms, presentation)?;
        }
        CatalogCommands::Show(args) => {
            let roms = load_catalog(source, presentation)?;
            let rom = find_by_name(&roms, &args.name)
                .with_context(|| format!("catalog entry '{}' was not found", args.name))?;
            render_roms(std::slice::from_ref(rom), presentation)?;
        }
    }
    Ok(())
}

fn run_category(
    command: CategoryCommands,
    source: &SourceOptions,
    presentation: &PresentationOptions,
) -> Result<()> {
    let roms = load_catalog(source, presentation)?;
    match command {
        CategoryCommands::List(args) => {
            let query = args.query.as_deref().map(str::to_ascii_lowercase);
            let mut counts = BTreeMap::<String, usize>::new();
            for rom in &roms {
                if let Some(genre) = &rom.metadata.genre
                    && query
                        .as_deref()
                        .is_none_or(|query| genre.category.to_ascii_lowercase().contains(query))
                {
                    *counts.entry(genre.category.clone()).or_default() += 1;
                }
            }
            let rows = counts
                .into_iter()
                .map(|(category, entries)| CategoryCount { category, entries })
                .collect::<Vec<_>>();
            render_category_counts(&rows, presentation)?;
        }
        CategoryCommands::Show(args) => {
            let category = args.category.to_ascii_lowercase();
            let subcategory = args.subcategory.map(|value| value.to_ascii_lowercase());
            let entries = roms
                .into_iter()
                .filter(|rom| {
                    rom.metadata.genre.as_ref().is_some_and(|genre| {
                        genre.category.eq_ignore_ascii_case(&category)
                            && subcategory.as_deref().is_none_or(|expected| {
                                genre.subcategory.as_deref().is_some_and(|actual| {
                                    actual.to_ascii_lowercase().contains(expected)
                                })
                            })
                    })
                })
                .collect::<Vec<_>>();
            if entries.is_empty() {
                bail!("category '{}' was not found", args.category);
            }
            render_roms(&entries, presentation)?;
        }
    }
    Ok(())
}

fn run_source(
    command: SourceCommands,
    source: &SourceOptions,
    presentation: &PresentationOptions,
) -> Result<()> {
    match command {
        SourceCommands::List => {
            let details = source_details(source);
            render_source_details(&details, presentation)?;
        }
        SourceCommands::Refresh(args) => {
            refresh_sources(args.target, source)?;
            let details = source_details(source);
            render_source_details(&details, presentation)?;
        }
        SourceCommands::Clear(args) => {
            let target = metadata_target(args.target);
            let entries = plan_clear_managed_cache(target)?;
            let mut operations = entries
                .iter()
                .map(|entry| OperationResult {
                    name: entry
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("cache")
                        .to_string(),
                    category: None,
                    subcategory: None,
                    source: entry.path.display().to_string(),
                    destination: None,
                    action: "clear".to_string(),
                    state: if args.execute { "executed" } else { "preview" }.to_string(),
                })
                .collect::<Vec<_>>();
            if args.execute {
                let mut failures = 0;
                for (entry, operation) in entries.iter().zip(&mut operations) {
                    if !entry.exists {
                        operation.state = "skipped".to_string();
                        continue;
                    }
                    let entry_target = if entry.path.file_name().and_then(|name| name.to_str())
                        == Some("mame.xml")
                    {
                        MetadataSourceTarget::Mame
                    } else {
                        MetadataSourceTarget::Catver
                    };
                    match clear_managed_cache(entry_target) {
                        Ok(_) => operation.state = "executed".to_string(),
                        Err(error) => {
                            operation.state = format!("error: {error}");
                            failures += 1;
                        }
                    }
                }
                render_operation_rows(&operations, presentation)?;
                if failures > 0 {
                    bail!("{failures} cache clear operation(s) failed");
                }
            } else {
                render_operation_rows(&operations, presentation)?;
            }
        }
    }
    Ok(())
}

fn scan_roms(
    rom_dir: &Path,
    source: &SourceOptions,
    presentation: &PresentationOptions,
) -> Result<Vec<RomEntry>> {
    let entries = load_catalog(source, presentation)?;
    status(presentation, &format!("Scanning {}", rom_dir.display()));
    scan_rom_folder_with_entries_and_progress(rom_dir, entries, |_, _| {}).map_err(Into::into)
}

fn load_catalog(
    source: &SourceOptions,
    presentation: &PresentationOptions,
) -> Result<Vec<RomEntry>> {
    status(presentation, "Resolving metadata sources");
    let mame = resolve_mame_xml(
        source.mame_xml.as_deref(),
        source.mame_executable.as_deref(),
    )?;
    let catver = resolve_catver(source.catver.as_deref())?;
    status(
        presentation,
        &format!(
            "Loading {} and {}",
            mame.path.display(),
            catver.path.display()
        ),
    );
    load_metadata(&mame.path, Some(&catver.path)).map_err(Into::into)
}

fn status(presentation: &PresentationOptions, message: &str) {
    if !presentation.quiet && !matches!(presentation.output, OutputFormat::Json) {
        eprintln!("{message}");
    }
}

fn matches_status(rom: &RomEntry, status: RomStatus) -> bool {
    match status {
        RomStatus::Available => rom.rom_path.is_some() && rom.catalogued,
        RomStatus::Missing => rom.rom_path.is_none() && rom.catalogued,
        RomStatus::Unmatched => rom.rom_path.is_some() && !rom.catalogued,
        RomStatus::All => true,
    }
}

fn select_roms(
    roms: Vec<RomEntry>,
    selectors: &SelectorArgs,
    include_non_runnable: bool,
) -> Result<Vec<RomEntry>> {
    validate_selectors(selectors)?;
    let year = selectors
        .year
        .as_deref()
        .map(parse_year_selector)
        .transpose()?;
    let (year_from, year_to) = year.map_or((None, None), |(start, end)| (Some(start), Some(end)));
    Ok(filter_roms(
        &roms,
        &FilterOptions {
            genre_contains: None,
            region: None,
            names: selectors.name.clone(),
            genres: selectors.genre.clone(),
            categories: selectors
                .category
                .iter()
                .map(|category| {
                    category
                        .trim()
                        .trim_matches(|character| character == '\'' || character == '"')
                        .trim()
                        .to_string()
                })
                .collect(),
            subcategories: selectors.subcategory.clone(),
            regions: selectors.region.iter().map(region_from_arg).collect(),
            manufacturers: selectors.manufacturer.clone(),
            year_from,
            year_to,
            include_mature: selectors.all || selectors.include_mature,
            include_mechanical: selectors.all || selectors.include_mechanical,
            include_prototype: selectors.all || selectors.include_prototype,
            include_non_runnable: selectors.all,
            include_uncatalogued: include_non_runnable,
            only_available: false,
        },
    ))
}

fn region_from_arg(region: &RegionArg) -> Region {
    match region {
        RegionArg::Usa => Region::Usa,
        RegionArg::Japan => Region::Japan,
        RegionArg::Europe => Region::Europe,
        RegionArg::World => Region::World,
        RegionArg::Asia => Region::Asia,
        RegionArg::Unknown => Region::Unknown,
    }
}

fn parse_year_selector(value: &str) -> Result<(u16, u16)> {
    if let Some((start, end)) = value.split_once("..") {
        let start = start
            .parse::<u16>()
            .with_context(|| format!("invalid start year '{start}'"))?;
        let end = end
            .parse::<u16>()
            .with_context(|| format!("invalid end year '{end}'"))?;
        if start > end {
            bail!("year range start must not be after its end");
        }
        Ok((start, end))
    } else {
        let year = value
            .parse::<u16>()
            .with_context(|| format!("invalid year '{value}'"))?;
        Ok((year, year))
    }
}

fn sort_roms(roms: &mut [RomEntry], ordering: &OrderingArgs) {
    roms.sort_by(|left, right| {
        let result = match ordering.sort {
            SortField::Name => left.name.cmp(&right.name),
            SortField::Year => left.year.cmp(&right.year),
            SortField::Manufacturer => left.manufacturer.cmp(&right.manufacturer),
            SortField::Category => category_of(left).cmp(&category_of(right)),
            SortField::Region => {
                region_name(&left.metadata.region).cmp(region_name(&right.metadata.region))
            }
        };
        if ordering.reverse {
            result.reverse()
        } else {
            result
        }
    });
}

fn category_of(rom: &RomEntry) -> Option<&str> {
    rom.metadata
        .genre
        .as_ref()
        .map(|genre| genre.category.as_str())
}

fn subcategory_of(rom: &RomEntry) -> Option<&str> {
    rom.metadata
        .genre
        .as_ref()
        .and_then(|genre| genre.subcategory.as_deref())
}

fn region_name(region: &Region) -> &str {
    match region {
        Region::Usa => "usa",
        Region::Japan => "japan",
        Region::Europe => "europe",
        Region::World => "world",
        Region::Asia => "asia",
        Region::Other(_) => "other",
        Region::Unknown => "unknown",
    }
}

fn require_mutation_selection(selectors: &SelectorArgs) -> Result<()> {
    validate_selectors(selectors)?;
    if !selectors.has_selection() {
        bail!("a mutation requires at least one selector or explicit --all");
    }
    Ok(())
}

fn validate_selectors(selectors: &SelectorArgs) -> Result<()> {
    let values = selectors
        .name
        .iter()
        .chain(&selectors.genre)
        .chain(&selectors.category)
        .chain(&selectors.subcategory)
        .chain(&selectors.manufacturer);
    if values.into_iter().any(|value| value.trim().is_empty()) {
        bail!("selector values must not be empty");
    }
    if selectors
        .year
        .as_deref()
        .is_some_and(|year| year.trim().is_empty())
    {
        bail!("year selector must not be empty");
    }
    Ok(())
}

fn require_matches(roms: &[RomEntry]) -> Result<()> {
    if roms.is_empty() {
        bail!("no available ROMs matched the requested selection");
    }
    Ok(())
}

#[derive(Serialize)]
struct OperationResult {
    name: String,
    category: Option<String>,
    subcategory: Option<String>,
    source: String,
    destination: Option<String>,
    action: String,
    state: String,
}

fn plan_move(roms: &[RomEntry], destination: &Path) -> Result<Vec<OperationResult>> {
    let mut targets = BTreeSet::new();
    let mut results = Vec::new();
    for rom in roms {
        let source = rom
            .rom_path
            .as_ref()
            .with_context(|| format!("ROM '{}' has no source path", rom.name))?;
        let file_name = source
            .file_name()
            .with_context(|| format!("ROM '{}' has no file name", rom.name))?;
        let target = destination.join(file_name);
        if !targets.insert(target.clone()) {
            bail!("multiple selected ROMs resolve to {}", target.display());
        }
        if target.try_exists()? {
            bail!("target already exists: {}", target.display());
        }
        results.push(OperationResult {
            name: rom.name.clone(),
            category: category_of(rom).map(ToOwned::to_owned),
            subcategory: subcategory_of(rom).map(ToOwned::to_owned),
            source: source.display().to_string(),
            destination: Some(target.display().to_string()),
            action: "move".to_string(),
            state: "preview".to_string(),
        });
    }
    Ok(results)
}

fn plan_delete(roms: &[RomEntry]) -> Result<Vec<OperationResult>> {
    roms.iter()
        .map(|rom| {
            let source = rom
                .rom_path
                .as_ref()
                .with_context(|| format!("ROM '{}' has no source path", rom.name))?;
            Ok(OperationResult {
                name: rom.name.clone(),
                category: category_of(rom).map(ToOwned::to_owned),
                subcategory: subcategory_of(rom).map(ToOwned::to_owned),
                source: source.display().to_string(),
                destination: None,
                action: "delete".to_string(),
                state: "preview".to_string(),
            })
        })
        .collect()
}

#[derive(Serialize)]
struct CategoryStats {
    total: usize,
    available: usize,
    missing: usize,
    unmatched: usize,
}

#[derive(Serialize)]
struct CollectionStats {
    total: usize,
    available: usize,
    missing: usize,
    unmatched: usize,
    by_category: BTreeMap<String, CategoryStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_roms: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unmatched_roms: Option<Vec<String>>,
}

impl CollectionStats {
    fn from_roms(roms: &[RomEntry], show_missing: bool, show_unmatched: bool) -> Self {
        let mut by_category = BTreeMap::new();
        for rom in roms {
            let category = category_of(rom).unwrap_or("Unknown").to_string();
            let stats = by_category.entry(category).or_insert(CategoryStats {
                total: 0,
                available: 0,
                missing: 0,
                unmatched: 0,
            });
            stats.total += 1;
            match (rom.rom_path.is_some(), rom.catalogued) {
                (true, true) => stats.available += 1,
                (false, true) => stats.missing += 1,
                (true, false) => stats.unmatched += 1,
                (false, false) => {}
            }
        }
        Self {
            total: roms.len(),
            available: roms
                .iter()
                .filter(|rom| rom.rom_path.is_some() && rom.catalogued)
                .count(),
            missing: roms
                .iter()
                .filter(|rom| rom.rom_path.is_none() && rom.catalogued)
                .count(),
            unmatched: roms
                .iter()
                .filter(|rom| rom.rom_path.is_some() && !rom.catalogued)
                .count(),
            by_category,
            missing_roms: show_missing.then(|| {
                roms.iter()
                    .filter(|rom| {
                        rom.rom_path.is_none() && rom.catalogued && rom.metadata.flags.runnable
                    })
                    .map(|rom| rom.name.clone())
                    .collect()
            }),
            unmatched_roms: show_unmatched.then(|| {
                roms.iter()
                    .filter(|rom| rom.rom_path.is_some() && !rom.catalogued)
                    .map(|rom| rom.name.clone())
                    .collect()
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum FindingLevel {
    Warning,
    Error,
}

impl PartialEq<AuditLevel> for FindingLevel {
    fn eq(&self, other: &AuditLevel) -> bool {
        matches!(
            (self, other),
            (Self::Warning, AuditLevel::Warning) | (Self::Error, AuditLevel::Error)
        )
    }
}

impl PartialOrd<AuditLevel> for FindingLevel {
    fn partial_cmp(&self, other: &AuditLevel) -> Option<Ordering> {
        Some(match (self, other) {
            (Self::Warning, AuditLevel::Warning) | (Self::Error, AuditLevel::Error) => {
                Ordering::Equal
            }
            (Self::Warning, AuditLevel::Error) => Ordering::Less,
            (Self::Error, AuditLevel::Warning) => Ordering::Greater,
        })
    }
}

#[derive(Serialize)]
struct AuditFinding {
    level: FindingLevel,
    kind: String,
    name: String,
    message: String,
}

fn audit_collection(rom_dir: &Path, roms: &[RomEntry]) -> Result<Vec<AuditFinding>> {
    let mut findings = Vec::new();
    for rom in roms {
        if rom.rom_path.is_some() && !rom.catalogued {
            findings.push(AuditFinding {
                level: FindingLevel::Error,
                kind: "unmatched".to_string(),
                name: rom.name.clone(),
                message: "archive has no matching runnable catalog entry".to_string(),
            });
        } else if rom.rom_path.is_none() && rom.catalogued && rom.metadata.flags.runnable {
            findings.push(AuditFinding {
                level: FindingLevel::Warning,
                kind: "missing".to_string(),
                name: rom.name.clone(),
                message: "catalog entry has no archive in the ROM directory".to_string(),
            });
        }
        if rom.rom_path.is_some() && rom.metadata.genre.is_none() && rom.catalogued {
            findings.push(AuditFinding {
                level: FindingLevel::Warning,
                kind: "metadata".to_string(),
                name: rom.name.clone(),
                message: "available ROM has no category metadata".to_string(),
            });
        }
    }

    let mut names = HashMap::<String, Vec<PathBuf>>::new();
    for path in list_rom_files(rom_dir)? {
        if let Some(name) = path.file_stem().and_then(|value| value.to_str()) {
            names
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(path);
        }
    }
    for (name, paths) in names.into_iter().filter(|(_, paths)| paths.len() > 1) {
        findings.push(AuditFinding {
            level: FindingLevel::Error,
            kind: "duplicate".to_string(),
            name,
            message: format!(
                "multiple archives resolve to the same ROM name: {}",
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
    findings.sort_by(|left, right| left.kind.cmp(&right.kind).then(left.name.cmp(&right.name)));
    Ok(findings)
}

#[derive(Serialize)]
struct CategoryCount {
    category: String,
    entries: usize,
}

#[derive(Serialize)]
struct SourceDetail {
    source: String,
    state: String,
    origin: String,
    path: Option<String>,
    size_bytes: Option<u64>,
    modified: Option<String>,
}

fn source_details(source: &SourceOptions) -> Vec<SourceDetail> {
    let paths = match managed_cache_paths() {
        Ok(paths) => paths,
        Err(error) => {
            return vec![
                unavailable_source("mame", error.to_string()),
                unavailable_source("catver", error.to_string()),
            ];
        }
    };
    let (mame_path, mame_origin) = if let Some(path) = &source.mame_xml {
        (path.as_path(), "explicit path".to_string())
    } else if let Some(executable) = &source.mame_executable {
        (
            paths.mame_xml.as_path(),
            format!(
                "managed cache; executable configured at {}",
                executable.display()
            ),
        )
    } else {
        (paths.mame_xml.as_path(), "managed cache".to_string())
    };
    let mame = detail_for_file("mame", mame_path, mame_origin);

    let (catver_path, catver_origin) = source
        .catver
        .as_deref()
        .map(|path| (path, "explicit path".to_string()))
        .unwrap_or((paths.catver.as_path(), "managed cache".to_string()));
    let catver = detail_for_file("catver", catver_path, catver_origin);
    vec![mame, catver]
}

fn detail_for_file(source: &str, path: &Path, origin: String) -> SourceDetail {
    match fs::metadata(path) {
        Ok(metadata) => SourceDetail {
            source: source.to_string(),
            state: "available".to_string(),
            origin,
            path: Some(path.display().to_string()),
            size_bytes: Some(metadata.len()),
            modified: metadata.modified().ok().map(format_modified_date),
        },
        Err(error) => unavailable_source(source, error.to_string()),
    }
}

fn unavailable_source(source: &str, error: String) -> SourceDetail {
    SourceDetail {
        source: source.to_string(),
        state: "unavailable".to_string(),
        origin: error,
        path: None,
        size_bytes: None,
        modified: None,
    }
}

fn format_modified_date(modified: SystemTime) -> String {
    let date: DateTime<Local> = modified.into();
    date.format("%Y-%m-%d %H:%M:%S %Z").to_string()
}

fn refresh_sources(target: SourceTarget, source: &SourceOptions) -> Result<()> {
    match target {
        SourceTarget::Mame => {
            let executable = source
                .mame_executable
                .as_deref()
                .context("source refresh mame requires --mame-executable")?;
            refresh_mame_xml(executable)?;
        }
        SourceTarget::Catver => {
            refresh_catver()?;
        }
        SourceTarget::All => {
            let executable = source
                .mame_executable
                .as_deref()
                .context("source refresh all requires --mame-executable")?;
            refresh_mame_xml(executable)?;
            refresh_catver()?;
        }
    }
    Ok(())
}

fn metadata_target(target: SourceTarget) -> MetadataSourceTarget {
    match target {
        SourceTarget::Mame => MetadataSourceTarget::Mame,
        SourceTarget::Catver => MetadataSourceTarget::Catver,
        SourceTarget::All => MetadataSourceTarget::All,
    }
}

#[derive(Serialize)]
struct RomOutput {
    name: String,
    description: Option<String>,
    year: Option<String>,
    manufacturer: Option<String>,
    category: Option<String>,
    subcategory: Option<String>,
    region: String,
    available: bool,
    catalogued: bool,
    mature: bool,
    mechanical: bool,
    prototype: bool,
    runnable: bool,
    path: Option<String>,
}

impl From<&RomEntry> for RomOutput {
    fn from(rom: &RomEntry) -> Self {
        Self {
            name: rom.name.clone(),
            description: rom.description.clone(),
            year: rom.year.clone(),
            manufacturer: rom.manufacturer.clone(),
            category: category_of(rom).map(ToOwned::to_owned),
            subcategory: rom
                .metadata
                .genre
                .as_ref()
                .and_then(|genre| genre.subcategory.clone()),
            region: region_name(&rom.metadata.region).to_string(),
            available: rom.rom_path.is_some(),
            catalogued: rom.catalogued,
            mature: rom.metadata.flags.mature,
            mechanical: rom.metadata.flags.mechanical,
            prototype: rom.metadata.flags.prototype,
            runnable: rom.metadata.flags.runnable,
            path: rom.rom_path.as_ref().map(|path| path.display().to_string()),
        }
    }
}

fn render_roms(roms: &[RomEntry], options: &PresentationOptions) -> Result<()> {
    let output = roms.iter().map(RomOutput::from).collect::<Vec<_>>();
    render(
        &output,
        &[
            "name",
            "description",
            "year",
            "manufacturer",
            "category",
            "subcategory",
            "region",
            "available",
            "path",
        ],
        output
            .iter()
            .map(|rom| {
                vec![
                    rom.name.clone(),
                    rom.description.clone().unwrap_or_default(),
                    rom.year.clone().unwrap_or_default(),
                    rom.manufacturer.clone().unwrap_or_default(),
                    rom.category.clone().unwrap_or_default(),
                    rom.subcategory.clone().unwrap_or_default(),
                    rom.region.clone(),
                    rom.available.to_string(),
                    rom.path.clone().unwrap_or_default(),
                ]
            })
            .collect(),
        options,
    )
}

fn render_stats(stats: &CollectionStats, options: &PresentationOptions) -> Result<()> {
    let mut rows = vec![
        vec!["total".to_string(), stats.total.to_string()],
        vec!["available".to_string(), stats.available.to_string()],
        vec!["missing".to_string(), stats.missing.to_string()],
        vec!["unmatched".to_string(), stats.unmatched.to_string()],
    ];
    rows.extend(
        stats
            .by_category
            .iter()
            .flat_map(|(category, category_stats)| {
                [
                    vec![
                        format!("category:{category}:total"),
                        category_stats.total.to_string(),
                    ],
                    vec![
                        format!("category:{category}:available"),
                        category_stats.available.to_string(),
                    ],
                    vec![
                        format!("category:{category}:missing"),
                        category_stats.missing.to_string(),
                    ],
                    vec![
                        format!("category:{category}:unmatched"),
                        category_stats.unmatched.to_string(),
                    ],
                ]
            }),
    );
    if let Some(missing_roms) = &stats.missing_roms {
        rows.extend(
            missing_roms
                .iter()
                .map(|name| vec!["missing_rom".to_string(), name.clone()]),
        );
    }
    if let Some(unmatched_roms) = &stats.unmatched_roms {
        rows.extend(
            unmatched_roms
                .iter()
                .map(|name| vec!["unmatched_rom".to_string(), name.clone()]),
        );
    }
    render(stats, &["metric", "count"], rows, options)
}

fn render_audit(findings: &[AuditFinding], options: &PresentationOptions) -> Result<()> {
    render(
        findings,
        &["level", "kind", "name", "message"],
        findings
            .iter()
            .map(|finding| {
                vec![
                    format!("{:?}", finding.level).to_ascii_lowercase(),
                    finding.kind.clone(),
                    finding.name.clone(),
                    finding.message.clone(),
                ]
            })
            .collect(),
        options,
    )
}

fn render_category_counts(rows: &[CategoryCount], options: &PresentationOptions) -> Result<()> {
    render(
        rows,
        &["category", "entries"],
        rows.iter()
            .map(|row| vec![row.category.clone(), row.entries.to_string()])
            .collect(),
        options,
    )
}

fn render_source_details(rows: &[SourceDetail], options: &PresentationOptions) -> Result<()> {
    render(
        rows,
        &[
            "source",
            "state",
            "origin",
            "path",
            "size_bytes",
            "modified",
        ],
        rows.iter()
            .map(|row| {
                vec![
                    row.source.clone(),
                    row.state.clone(),
                    row.origin.clone(),
                    row.path.clone().unwrap_or_default(),
                    row.size_bytes
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    row.modified.clone().unwrap_or_default(),
                ]
            })
            .collect(),
        options,
    )
}

fn render_operation_rows(rows: &[OperationResult], options: &PresentationOptions) -> Result<()> {
    render(
        rows,
        &[
            "name",
            "category",
            "subcategory",
            "source",
            "destination",
            "action",
            "state",
        ],
        rows.iter()
            .map(|row| {
                vec![
                    row.name.clone(),
                    row.category.clone().unwrap_or_default(),
                    row.subcategory.clone().unwrap_or_default(),
                    row.source.clone(),
                    row.destination.clone().unwrap_or_default(),
                    row.action.clone(),
                    row.state.clone(),
                ]
            })
            .collect(),
        options,
    )
}

fn render<T: Serialize + ?Sized>(
    value: &T,
    headers: &[&str],
    rows: Vec<Vec<String>>,
    options: &PresentationOptions,
) -> Result<()> {
    match options.output {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(value)?),
        OutputFormat::Tsv => {
            if !options.no_header {
                println!("{}", headers.join("\t"));
            }
            for row in rows {
                println!(
                    "{}",
                    row.iter()
                        .map(|cell| cell.replace(['\t', '\r', '\n'], " "))
                        .collect::<Vec<_>>()
                        .join("\t")
                );
            }
        }
        OutputFormat::Table => println!("{}", format_table(headers, rows, options.no_header)),
    }
    Ok(())
}

fn format_table(headers: &[&str], rows: Vec<Vec<String>>, no_header: bool) -> String {
    let widths = headers
        .iter()
        .enumerate()
        .map(|(column, header)| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|value| value.chars().count())
                .fold(
                    if no_header { 0 } else { header.chars().count() },
                    usize::max,
                )
        })
        .collect::<Vec<_>>();
    if widths.iter().all(|width| *width == 0) {
        return String::new();
    }
    let separator = format!(
        "+{}+",
        widths
            .iter()
            .map(|width| "-".repeat(width + 2))
            .collect::<Vec<_>>()
            .join("+")
    );
    let mut output = vec![separator.clone()];
    if !no_header {
        output.push(format_table_row(headers.iter().copied(), &widths));
        output.push(separator.clone());
    }
    output.extend(
        rows.iter()
            .map(|row| format_table_row(row.iter().map(String::as_str), &widths)),
    );
    output.push(separator);
    output.join("\n")
}

fn format_table_row<'a>(cells: impl Iterator<Item = &'a str>, widths: &[usize]) -> String {
    format!(
        "| {} |",
        cells
            .zip(widths)
            .map(|(cell, width)| format!("{cell:<width$}", width = width))
            .collect::<Vec<_>>()
            .join(" | ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn logo_uses_the_four_color_ascii_art() {
        for color in [39, 45, 51, 87] {
            assert!(
                LOGO.contains(&format!("\x1b[38;5;{color}m")),
                "missing color {color}"
            );
        }
        assert!(LOGO.contains(r"\________(____"));
        assert!(LOGO.ends_with("\x1b[0m"));
    }

    #[test]
    fn parses_years_and_ranges() {
        assert_eq!(parse_year_selector("1980").unwrap(), (1980, 1980));
        assert_eq!(parse_year_selector("1980..1985").unwrap(), (1980, 1985));
        assert!(parse_year_selector("1985..1980").is_err());
    }

    #[test]
    fn builds_client_install_commands_with_fixed_stdio_configuration() {
        let source = SourceOptions {
            mame_xml: Some(PathBuf::from("mame.xml")),
            catver: Some(PathBuf::from("catver.ini")),
            ..Default::default()
        };
        let server_arguments = mcp_server_arguments(Path::new("roms"), &source);
        let executable = Path::new("arcadejanitor-mcp");

        let vscode = mcp_install_command(McpSystem::VsCode, executable, &server_arguments).unwrap();
        assert_eq!(vscode.program, OsString::from("code"));
        assert_eq!(vscode.arguments[0], OsString::from("--add-mcp"));
        let configuration: serde_json::Value =
            serde_json::from_str(&vscode.arguments[1].to_string_lossy()).unwrap();
        assert_eq!(configuration["name"], "arcadejanitor");
        assert_eq!(configuration["type"], "stdio");
        assert_eq!(configuration["command"], "arcadejanitor-mcp");
        assert_eq!(
            configuration["args"],
            json!([
                "--transport",
                "stdio",
                "--rom-folder",
                "roms",
                "--mame-xml",
                "mame.xml",
                "--catver",
                "catver.ini",
            ])
        );

        let vscode_insiders =
            mcp_install_command(McpSystem::VsCodeInsiders, executable, &server_arguments).unwrap();
        assert_eq!(vscode_insiders.program, OsString::from("code-insiders"));
        assert_eq!(vscode_insiders.arguments, vscode.arguments);

        let copilot =
            mcp_install_command(McpSystem::CopilotCli, executable, &server_arguments).unwrap();
        assert_eq!(copilot.program, OsString::from("copilot"));
        assert_eq!(
            copilot.arguments,
            vec![
                "mcp",
                "add",
                "--transport",
                "stdio",
                "arcadejanitor",
                "--",
                "arcadejanitor-mcp",
                "--transport",
                "stdio",
                "--rom-folder",
                "roms",
                "--mame-xml",
                "mame.xml",
                "--catver",
                "catver.ini",
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
        );

        let claude =
            mcp_install_command(McpSystem::ClaudeCode, executable, &server_arguments).unwrap();
        assert_eq!(claude.program, OsString::from("claude"));
        assert_eq!(
            &claude.arguments[..7],
            [
                OsString::from("mcp"),
                OsString::from("add"),
                OsString::from("--scope"),
                OsString::from("user"),
                OsString::from("--transport"),
                OsString::from("stdio"),
                OsString::from("arcadejanitor"),
            ]
        );
    }

    #[test]
    fn forwards_the_mame_executable_to_installed_servers() {
        let source = SourceOptions {
            mame_executable: Some(PathBuf::from("mame")),
            ..Default::default()
        };

        assert_eq!(
            mcp_server_arguments(Path::new("roms"), &source),
            vec![
                "--transport",
                "stdio",
                "--rom-folder",
                "roms",
                "--mame-executable",
                "mame",
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
        );
    }
}
