use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(
    name = "cleanmame",
    version,
    about = "Inspect and manage MAME ROM collections",
    subcommand_required = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(flatten, next_help_heading = "Metadata source options")]
    pub source: SourceOptions,
    #[command(flatten, next_help_heading = "Output and diagnostics")]
    pub presentation: PresentationOptions,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args, Clone, Debug, Default)]
pub struct SourceOptions {
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        conflicts_with = "mame_executable",
        help = "Read MAME metadata from this XML file [env: CLEANMAME_MAME_XML]"
    )]
    pub mame_xml: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Extract MAME metadata using this executable [env: CLEANMAME_MAME_EXECUTABLE]"
    )]
    pub mame_executable: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Read category metadata from this catver.ini file [env: CLEANMAME_CATVER]"
    )]
    pub catver: Option<PathBuf>,
}

impl SourceOptions {
    pub fn apply_environment(mut self) -> Self {
        if self.mame_xml.is_none() {
            self.mame_xml = non_empty_env("CLEANMAME_MAME_XML").map(PathBuf::from);
        }
        if self.mame_executable.is_none() {
            self.mame_executable = non_empty_env("CLEANMAME_MAME_EXECUTABLE").map(PathBuf::from);
        }
        if self.catver.is_none() {
            self.catver = non_empty_env("CLEANMAME_CATVER").map(PathBuf::from);
        }
        self
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[derive(Args, Clone, Debug)]
pub struct PresentationOptions {
    #[arg(
        short,
        long,
        global = true,
        value_enum,
        default_value_t = OutputFormat::Table,
        help = "Output format"
    )]
    pub output: OutputFormat,
    #[arg(
        long,
        global = true,
        help = "Omit column headings from table and TSV output"
    )]
    pub no_header: bool,
    #[arg(
        short,
        long,
        global = true,
        help = "Suppress progress and diagnostic output"
    )]
    pub quiet: bool,
    #[arg(
        short,
        long,
        global = true,
        action = clap::ArgAction::Count,
        help = "Increase diagnostic verbosity"
    )]
    pub verbose: u8,
    #[arg(
        long,
        global = true,
        value_enum,
        default_value_t = ColorMode::Auto,
        help = "Control colored output"
    )]
    pub color: ColorMode,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Tsv,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Inspect and manage ROM archives in a directory")]
    Rom(RomCommand),
    #[command(about = "Browse the merged MAME metadata catalog")]
    Catalog(CatalogCommand),
    #[command(about = "Browse ROM categories and subcategories")]
    Category(CategoryCommand),
    #[command(about = "Inspect and manage metadata sources")]
    Source(SourceCommand),
    #[command(about = "Generate shell completion scripts")]
    Completions(CompletionsArgs),
}

#[derive(Args, Debug)]
pub struct RomCommand {
    #[command(subcommand)]
    pub command: RomCommands,
}

#[derive(Subcommand, Debug)]
pub enum RomCommands {
    #[command(about = "List ROMs in a directory")]
    List(RomListArgs),
    #[command(about = "Show one ROM in a directory")]
    Show(RomShowArgs),
    #[command(about = "Preview or execute moving selected ROMs")]
    Move(RomMoveArgs),
    #[command(about = "Preview or execute deleting selected ROMs")]
    Delete(RomDeleteArgs),
    #[command(about = "Report summary statistics for a ROM collection")]
    Stats(RomStatsArgs),
    #[command(about = "Audit a ROM collection for actionable problems")]
    Audit(RomAuditArgs),
}

#[derive(Args, Debug)]
pub struct RomListArgs {
    #[arg(value_name = "ROM_DIR", help = "Directory containing ROM archives")]
    pub rom_dir: PathBuf,
    #[command(flatten, next_help_heading = "Selectors")]
    pub selectors: SelectorArgs,
    #[arg(long, value_enum, default_value_t = RomStatus::Available)]
    pub status: RomStatus,
    #[command(flatten)]
    pub ordering: OrderingArgs,
}

#[derive(Args, Debug)]
pub struct RomShowArgs {
    #[arg(value_name = "ROM_DIR", help = "Directory containing ROM archives")]
    pub rom_dir: PathBuf,
    #[arg(value_name = "NAME", help = "Exact ROM name")]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct RomMoveArgs {
    #[arg(value_name = "ROM_DIR", help = "Directory containing ROM archives")]
    pub rom_dir: PathBuf,
    #[arg(
        value_name = "DESTINATION",
        help = "Directory to move selected archives into"
    )]
    pub destination: PathBuf,
    #[command(flatten, next_help_heading = "Selectors")]
    pub selectors: SelectorArgs,
    #[arg(
        long,
        help = "Perform the move; without this flag only a preview is shown"
    )]
    pub execute: bool,
}

#[derive(Args, Debug)]
pub struct RomDeleteArgs {
    #[arg(value_name = "ROM_DIR", help = "Directory containing ROM archives")]
    pub rom_dir: PathBuf,
    #[command(flatten, next_help_heading = "Selectors")]
    pub selectors: SelectorArgs,
    #[arg(
        long,
        help = "Perform deletion; without this flag only a preview is shown"
    )]
    pub execute: bool,
}

#[derive(Args, Debug)]
pub struct RomStatsArgs {
    #[arg(value_name = "ROM_DIR", help = "Directory containing ROM archives")]
    pub rom_dir: PathBuf,
    #[command(flatten, next_help_heading = "Selectors")]
    pub selectors: SelectorArgs,
    #[arg(long, help = "Include the names of missing ROMs in the report")]
    pub show_missing: bool,
}

#[derive(Args, Debug)]
pub struct RomAuditArgs {
    #[arg(value_name = "ROM_DIR", help = "Directory containing ROM archives")]
    pub rom_dir: PathBuf,
    #[arg(
        long,
        value_enum,
        default_value_t = AuditLevel::Warning,
        help = "Exit unsuccessfully when findings reach this level"
    )]
    pub fail_on: AuditLevel,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, ValueEnum)]
pub enum AuditLevel {
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum RomStatus {
    Available,
    Missing,
    Unmatched,
    All,
}

#[derive(Args, Debug)]
pub struct CatalogCommand {
    #[command(subcommand)]
    pub command: CatalogCommands,
}

#[derive(Subcommand, Debug)]
pub enum CatalogCommands {
    #[command(about = "List entries in the merged metadata catalog")]
    List(CatalogListArgs),
    #[command(about = "Show one entry in the merged metadata catalog")]
    Show(CatalogShowArgs),
}

#[derive(Args, Debug)]
pub struct CatalogListArgs {
    #[command(flatten, next_help_heading = "Selectors")]
    pub selectors: SelectorArgs,
    #[command(flatten)]
    pub ordering: OrderingArgs,
}

#[derive(Args, Debug)]
pub struct CatalogShowArgs {
    #[arg(value_name = "NAME", help = "Exact catalog entry name")]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct CategoryCommand {
    #[command(subcommand)]
    pub command: CategoryCommands,
}

#[derive(Subcommand, Debug)]
pub enum CategoryCommands {
    #[command(about = "List categories and their catalog entry counts")]
    List(CategoryListArgs),
    #[command(about = "Show subcategories and entries for one category")]
    Show(CategoryShowArgs),
}

#[derive(Args, Debug)]
pub struct CategoryListArgs {
    #[arg(long, value_name = "TEXT", help = "Match category names")]
    pub query: Option<String>,
}

#[derive(Args, Debug)]
pub struct CategoryShowArgs {
    #[arg(value_name = "CATEGORY", help = "Exact category name")]
    pub category: String,
    #[arg(long, value_name = "TEXT", help = "Match subcategory names")]
    pub subcategory: Option<String>,
}

#[derive(Args, Debug)]
pub struct SourceCommand {
    #[command(subcommand)]
    pub command: SourceCommands,
}

#[derive(Subcommand, Debug)]
pub enum SourceCommands {
    #[command(about = "List resolved metadata source details")]
    List,
    #[command(about = "Refresh managed metadata caches")]
    Refresh(SourceTargetArgs),
    #[command(about = "Preview or execute clearing managed metadata caches")]
    Clear(SourceClearArgs),
}

#[derive(Args, Debug)]
pub struct SourceTargetArgs {
    #[arg(value_enum, default_value_t = SourceTarget::All)]
    pub target: SourceTarget,
}

#[derive(Args, Debug)]
pub struct SourceClearArgs {
    #[arg(value_enum)]
    pub target: SourceTarget,
    #[arg(
        long,
        help = "Clear the cache; without this flag only a preview is shown"
    )]
    pub execute: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SourceTarget {
    Mame,
    Catver,
    All,
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    #[arg(value_enum)]
    pub shell: Shell,
}

#[derive(Args, Clone, Debug, Default)]
pub struct SelectorArgs {
    #[arg(long, value_name = "PATTERN", help = "Match a ROM name; repeat for OR")]
    pub name: Vec<String>,
    #[arg(long, value_name = "TEXT", help = "Match a genre; repeat for OR")]
    pub genre: Vec<String>,
    #[arg(
        long,
        value_name = "TEXT",
        value_delimiter = ',',
        help = "Match categories; separate values with commas or repeat for OR"
    )]
    pub category: Vec<String>,
    #[arg(long, value_name = "TEXT", help = "Match a subcategory; repeat for OR")]
    pub subcategory: Vec<String>,
    #[arg(long, value_enum, help = "Match a region; repeat for OR")]
    pub region: Vec<RegionArg>,
    #[arg(
        long,
        value_name = "TEXT",
        help = "Match a manufacturer; repeat for OR"
    )]
    pub manufacturer: Vec<String>,
    #[arg(
        long,
        value_name = "YEAR|START..END",
        help = "Match a year or inclusive range"
    )]
    pub year: Option<String>,
    #[arg(long, help = "Include mature entries")]
    pub include_mature: bool,
    #[arg(long, help = "Include mechanical entries")]
    pub include_mechanical: bool,
    #[arg(long, help = "Include prototype entries")]
    pub include_prototype: bool,
    #[arg(
        long,
        help = "Include normally excluded classes; explicitly select every ROM for mutations"
    )]
    pub all: bool,
}

impl SelectorArgs {
    pub fn has_selection(&self) -> bool {
        self.all
            || !self.name.is_empty()
            || !self.genre.is_empty()
            || !self.category.is_empty()
            || !self.subcategory.is_empty()
            || !self.region.is_empty()
            || !self.manufacturer.is_empty()
            || self.year.is_some()
    }
}

#[derive(Clone, Debug, ValueEnum)]
pub enum RegionArg {
    Usa,
    Japan,
    Europe,
    World,
    Asia,
    Unknown,
}

#[derive(Args, Debug)]
pub struct OrderingArgs {
    #[arg(long, value_enum, default_value_t = SortField::Name)]
    pub sort: SortField,
    #[arg(long, help = "Reverse the selected sort order")]
    pub reverse: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SortField {
    Name,
    Year,
    Manufacturer,
    Category,
    Region,
}
