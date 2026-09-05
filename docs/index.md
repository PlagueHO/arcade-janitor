---
layout: home
hero:
  name: ArcadeJanitor
  text: MAME ROM collection management
  tagline: A focused Rust CLI and MCP server for cleaning and understanding MAME ROM folders.
  image:
    src: /images/arcadejanitor.png
    alt: ArcadeJanitor logo
  actions:
    - theme: brand
      text: Install ArcadeJanitor
      link: /installation
    - theme: alt
      text: CLI Reference
      link: /cli
    - theme: alt
      text: MCP Server Setup
      link: /mcp
features:
  - title: Focused collection cleanup
    details: Audit, filter, move, and delete MAME ROM archives with metadata-aware previews before changes are made.
  - title: Metadata without setup friction
    details: Use local mame.xml and catver.ini files, or let ArcadeJanitor retrieve and cache the metadata it needs.
  - title: Agent-ready MCP support
    details: Connect a single, explicitly configured ROM folder to VS Code, GitHub Copilot CLI, or Claude Code.
---

## Why ArcadeJanitor?

ArcadeJanitor manages a main MAME ROM directory using only `mame.xml` and
`catver.ini` metadata. It is designed for the focused task of cleaning a ROM
folder without the complexity of broader ROM-management suites.

Use it directly from a terminal, or give an AI client access through its Model
Context Protocol (MCP) server. The MCP server is deliberately bound to one ROM
folder and metadata configuration, preventing an agent from operating on an
unintended collection.

## What it covers

ArcadeJanitor supports catalog queries, category browsing, collection auditing,
statistics, filtering, moving and deleting ROMs, metadata cache management, and
shell-completion generation. It intentionally does not support CHDs, BIOS
dependency resolution, scraping, a graphical user interface, cloud sync,
plugins, or non-MAME systems.

## Next steps

- [Install ArcadeJanitor](./installation.md) from the latest release.
- Learn the [CLI command structure and safety model](./cli.md).
- Configure the [MCP server](./mcp.md) for an agentic development system.
