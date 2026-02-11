<p align="center">
  <h1 align="center">ctx</h1>
  <p align="center"><strong>Universal Agent Context Protocol</strong></p>
  <p align="center">Live codebase intelligence for AI agents. Zero dependencies, local-first, offline-capable.</p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-stable-orange" alt="Rust">
  <img src="https://img.shields.io/badge/sqlite-FTS5-blue" alt="SQLite">
  <img src="https://img.shields.io/badge/MCP-compatible-green" alt="MCP">
  <img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License">
</p>

---

## What is ctx?

**ctx** is a Rust CLI tool that gives AI agents deep, structured understanding of any codebase. It scans your project, extracts symbols using tree-sitter, maps dependencies, analyzes git history, and stores everything in a single local SQLite file — queryable via CLI or MCP protocol.

**No LLM required. No cloud. No API keys. Just intelligence.**

```
$ ctx init
  ctx — Universal Agent Context Protocol

  ✓ Created .ctx/ctx.db
  ⟳ Scanning project... done
    16 files discovered
    109 symbols extracted
    54 dependencies mapped
  ⟳ Analyzing git history... done
    4 commits analyzed
    2 decisions extracted

  ✓ Initialized in 1.7s
```

## Features

| Feature | Description |
|---------|-------------|
| **📦 Codebase Map** | Directory tree with file counts, line counts, and symbols per file |
| **🔣 Symbol Extraction** | Functions, classes, structs, interfaces, enums, methods — with full signatures |
| **🔗 Dependency Graph** | Import/export analysis with blast radius calculation |
| **📋 Decision Tracking** | Auto-extracts decisions from conventional commits |
| **🔍 Full-Text Search** | FTS5-powered search across all symbols |
| **📊 Health Warnings** | Fragile files, dead code, large file detection |
| **🧠 Knowledge Notes** | Store architectural insights and gotchas |
| **👁 File Watcher** | Live re-analysis on file changes |
| **🤖 MCP Server** | AI agents connect via Model Context Protocol |

## Supported Languages

| Language | Symbols | Imports |
|----------|---------|---------|
| Rust | ✅ Functions, Structs, Enums, Impls, Modules | ✅ `use` statements |
| TypeScript/JavaScript | ✅ Functions, Classes, Interfaces, Types | ✅ `import` statements |
| Python | ✅ Functions, Classes | ✅ `import`/`from` statements |
| Go, Java, C, C++, Ruby, PHP, Swift, Kotlin, Scala, Zig, Elixir, Haskell, OCaml, Lua, Dart, R, Julia, Dockerfile, Makefile, Shell | ✅ File tracking, line counts | ❌ (tree-sitter grammars not yet added) |

## Quick Start

### Build from source

```bash
git clone <repo-url> && cd ctx
cargo build --release
```

The binary will be at `./target/release/ctx`.

### Initialize a project

```bash
cd your-project
ctx init
```

This creates `.ctx/ctx.db` with all codebase intelligence.

### Explore

```bash
# Project overview
ctx status

# Directory tree with symbols
ctx map

# Search for symbols
ctx query "parse"

# Impact analysis
ctx blast-radius src/db/mod.rs

# View decisions from git history
ctx decisions

# Add a knowledge note
ctx learn "Auth module uses JWT with RS256"

# Show warnings (fragile files, dead code)
ctx warnings

# Live re-analysis on changes
ctx watch
```

## CLI Reference

```
Usage: ctx [OPTIONS] <COMMAND>

Commands:
  init          Initialize ctx in the current project
  scan          Scan/re-scan the project (incremental)
  map           Display codebase map with structure and stats
  status        Show project status dashboard
  query         Search symbols and files (FTS5)
  blast-radius  Show blast radius of changing a file
  decisions     Show recorded decisions
  learn         Add a knowledge note
  warnings      Show warnings (fragile files, dead code)
  watch         Watch for file changes and re-analyze

Options:
  -p, --project <PROJECT>  Project root directory (defaults to cwd)
  -h, --help               Print help
  -V, --version            Print version
```

## MCP Server

ctx includes a TypeScript MCP server that exposes all functionality to AI agents via the [Model Context Protocol](https://modelcontextprotocol.io/).

### Setup

```bash
cd mcp-server
npm install
npm run build
```

### Configure

Add to your MCP config (e.g. `mcp_config.json`):

```json
{
  "mcpServers": {
    "ctx": {
      "command": "node",
      "args": ["/path/to/ctx/mcp-server/dist/index.js"]
    }
  }
}
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `ctx_init` | Initialize ctx in a project |
| `ctx_status` | Project dashboard |
| `ctx_map` | Codebase structure map |
| `ctx_scan` | Incremental re-scan |
| `ctx_query` | Full-text symbol search |
| `ctx_blast_radius` | File impact analysis |
| `ctx_decisions` | Decision history |
| `ctx_learn` | Store knowledge notes |
| `ctx_warnings` | Codebase health warnings |

## Architecture

```
ctx/
├── src/
│   ├── main.rs          # CLI (clap)
│   ├── lib.rs           # Module exports
│   ├── db/
│   │   ├── mod.rs       # SQLite + FTS5 operations
│   │   ├── models.rs    # Data models
│   │   └── schema.rs    # Schema migrations
│   ├── analyzer/
│   │   ├── mod.rs       # Orchestrator
│   │   ├── scanner.rs   # File discovery
│   │   ├── parser.rs    # tree-sitter extraction
│   │   └── graph.rs     # Dependency graph
│   ├── git/
│   │   ├── mod.rs       # Git module
│   │   └── history.rs   # Commit analysis
│   ├── query/
│   │   ├── search.rs    # FTS5 search
│   │   └── blast.rs     # Blast radius
│   └── watcher/
│       └── mod.rs       # File watcher daemon
└── mcp-server/
    ├── src/index.ts     # TypeScript MCP server
    ├── tsconfig.json
    └── package.json
```

## How It Works

1. **Scan** — Walks the project directory respecting `.gitignore`, detects languages, computes file hashes
2. **Parse** — Uses tree-sitter to extract symbols and imports from supported languages
3. **Store** — Everything goes into a single SQLite file (`.ctx/ctx.db`) with WAL mode
4. **Index** — FTS5 virtual table indexes all symbols for instant search
5. **Analyze** — Git history provides churn scores, contributor data, and decision extraction
6. **Serve** — CLI or MCP protocol for AI agent integration

## Design Principles

- **🔒 Local-first**: All data stays on your machine in a single `.ctx/ctx.db` file
- **📡 Offline-capable**: No internet, no API keys, no cloud — works anywhere
- **⚡ Incremental**: File hashes track changes — only re-analyzes what changed
- **🪶 Zero runtime deps**: Single binary, no Docker, no services to run
- **🤖 Agent-native**: Built for AI agents via MCP, not just humans

## License

MIT
