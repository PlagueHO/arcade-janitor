# CLI Reference

ArcadeJanitor uses a resource-first command structure:

```text
arcadejanitor
├── rom        list, show, move, delete, stats, audit
├── catalog    list, show
├── category   list, show
├── source     list, refresh, clear
├── mcp        start, install
└── completions
```

Run `arcadejanitor <resource> --help` and
`arcadejanitor <resource> <command> --help` to discover the available options.

## ROM collection commands

Use `rom list` and `rom show` to inspect a folder, `rom stats` to report
collection totals, and `rom audit` to identify actionable problems:

```bash
./arcadejanitor rom list ./roms --genre maze --mame-xml ./mame.xml --catver ./catver.ini
./arcadejanitor rom show ./roms --name pacman --mame-xml ./mame.xml --catver ./catver.ini
./arcadejanitor rom stats ./roms --show-missing --show-unmatched --output json --mame-xml ./mame.xml
./arcadejanitor rom audit ./roms --mame-xml ./mame.xml
```

Filter ROM commands with repeatable `--name`, `--genre`, `--category`,
`--subcategory`, `--region`, and `--manufacturer` options. `--year` accepts a
single year or inclusive range such as `1980..1985`. Separate category values
with commas or repeat `--category` to select any of them.

## Moving and deleting ROMs safely

`rom move` and `rom delete` preview their operations by default. Add `--execute`
only after reviewing the preview. Both mutation commands require at least one
selector or explicit `--all`.

```bash
./arcadejanitor rom move ./roms --destination ./filtered --genre maze --mame-xml ./mame.xml
./arcadejanitor rom move ./roms --destination ./filtered --genre maze --execute --mame-xml ./mame.xml
./arcadejanitor rom delete ./roms --name "prototype*" --execute --mame-xml ./mame.xml
```

## Catalog and category commands

The catalog combines MAME and category metadata. Query individual entries or
browse categories and subcategories:

```bash
./arcadejanitor catalog show --name pacman --mame-xml ./mame.xml --catver ./catver.ini --output json
./arcadejanitor catalog list --manufacturer Namco --year 1980..1985 --mame-xml ./mame.xml
./arcadejanitor category list --query Shooter --mame-xml ./mame.xml
./arcadejanitor category show --category Shooter --subcategory "Flying Vertical" --mame-xml ./mame.xml
```

## Output and metadata cache commands

List-like commands support table, JSON, and TSV output through `--output`.
Use `--no-header` for headerless table or TSV output. Results are written to
stdout; progress and diagnostics are written to stderr. Use `--quiet` to
suppress diagnostics or repeat `--verbose` for additional detail.

Inspect or maintain metadata caches with:

```bash
./arcadejanitor source list --output table
./arcadejanitor source refresh --target all
./arcadejanitor source clear --target catver
./arcadejanitor source clear --target catver --execute
```

Like ROM mutations, `source clear` previews its operation until `--execute` is
specified. Generate shell completion scripts with
`arcadejanitor completions --shell powershell`, replacing `powershell` with your shell.
