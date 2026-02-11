<p align="center">
  <h1 align="center">ctx-agent</h1>
  <p align="center"><strong>Agent Context Protocol</strong></p>
  <p align="center">Structured codebase intelligence for AI agents.<br/>Local-first. Offline-capable. Zero dependencies.</p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-stable-orange" alt="Rust">
  <img src="https://img.shields.io/badge/sqlite-FTS5-blue" alt="SQLite">
  <img src="https://img.shields.io/badge/MCP-compatible-green" alt="MCP">
  <img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License">
</p>

---

> **One-liner:** ctx-agent lets an AI agent answer *"if I change this file, what breaks?"* without running the code.

---

## What is ctx-agent?

**ctx-agent** is a Rust CLI that gives AI agents deep, structured understanding of any codebase. It scans your project, extracts symbols using tree-sitter, maps dependencies, analyzes git history, and stores everything in a single local SQLite file — queryable via CLI or [MCP protocol](https://modelcontextprotocol.io/).

**No LLM required. No cloud. No API keys. Just intelligence.**

```
$ ctx-agent init
  ctx-agent — Agent Context Protocol

  ✓ Created .ctx/ctx.db
  ⟳ Scanning project... done
    21 files discovered
    119 symbols extracted
    61 dependencies mapped
  ⟳ Analyzing git history... done
    8 commits analyzed
    3 decisions extracted

  ✓ Initialized in 0.1s
```

## What ctx-agent is NOT

- **Not an LLM** — It doesn't generate code or answers. It provides structured context that agents consume.
- **Not a linter/compiler** — It doesn't guarantee semantic correctness. It reads structure, not behavior.
- **Not a runtime analyzer** — No execution tracing, profiling, or dynamic analysis. Purely static.
- **Not a Language Server** — No code completion, go-to-definition, or refactoring. Different problem space.

## Features

| Feature | Description |
|---------|-------------|
| 📦 **Codebase Map** | Directory tree with file counts, line counts, and symbols per file |
| 🔣 **Symbol Extraction** | Functions, classes, structs, interfaces, enums — with full signatures |
| 🔗 **Dependency Graph** | Import/export analysis with blast radius calculation |
| 📋 **Decision Tracking** | Auto-extracts decisions from conventional commits |
| 🔍 **Full-Text Search** | FTS5-powered symbol search with partial matching |
| 📊 **Health Warnings** | Fragile files, dead code, large file detection |
| 🧠 **Knowledge Notes** | Store architectural insights and gotchas |
| 👁 **File Watcher** | Live re-analysis on file changes |
| 🤖 **MCP Server** | AI agents connect via Model Context Protocol |
| 📤 **JSON Output** | Machine-readable output for agent consumption |

## How ctx-agent Compares

| Feature | ctx-agent | ctags/LSP | Sourcegraph | Copilot Context |
|---------|-----------|-----------|-------------|-----------------|
| Local-first | ✅ | ✅ | ❌ (server) | ❌ (cloud) |
| Agent-native (MCP) | ✅ | ❌ | ❌ | ❌ |
| Offline | ✅ | ✅ | ❌ | ❌ |
| Incremental scan | ✅ | ✅ | ❌ | N/A |
| Blast radius | ✅ | ❌ | ✅ | ❌ |
| Decision tracking | ✅ | ❌ | ❌ | ❌ |
| Single portable file | ✅ (SQLite) | ✅ (tags) | ❌ | ❌ |
| Health warnings | ✅ | ❌ | ❌ | ❌ |

**ctx-agent fills a specific gap:** giving AI agents codebase memory without cloud, servers, or LLMs.

## Language Support

| Language | Symbols | Imports | Status |
|----------|---------|---------|--------|
| **Rust** | ✅ Functions, Structs, Enums, Impls, Modules | ✅ `use` statements | Full |
| **TypeScript/JavaScript** | ✅ Functions, Classes, Interfaces, Types | ✅ `import`/`export` | Full |
| **Python** | ✅ Functions, Classes, Decorators | ✅ `import`/`from` | Full |
| Go, Java, C/C++, Ruby, PHP, Swift, Kotlin | 📄 File tracking + line counts | ❌ | Planned |

> **Note:** Languages without symbol extraction still get file tracking, dependency counting via file references, and git history analysis.

## Quick Start

### Build from source

```bash
git clone https://github.com/ahmetshbz1/ctx-agent.git && cd ctx-agent
cargo build --release
```

### Initialize a project

```bash
cd your-project
ctx-agent init
```

This creates `.ctx/ctx.db` with all codebase intelligence.

### Explore

```bash
# Project overview
ctx-agent status

# Directory tree with symbols
ctx-agent map

# Search for symbols
ctx-agent query "parse"

# Impact analysis
ctx-agent blast-radius src/db/mod.rs

# View decisions from git history
ctx-agent decisions

# Add a knowledge note
ctx-agent learn "Auth module uses JWT with RS256"

# Show warnings (fragile files, dead code)
ctx-agent warnings

# Live re-analysis on changes
ctx-agent watch

# JSON output for agents
ctx-agent status --json
ctx-agent query "parse" --json
```

## Decision Tracking

ctx-agent extracts architectural decisions from your git history using [conventional commits](https://www.conventionalcommits.org/):

```
$ ctx-agent decisions

  📋 3 decisions

  2026-02-10 [commit] feat(auth): switch to JWT RS256 (a3b8d1)
  2026-02-10 [commit] fix: FTS5 contentless table — use regular FTS5 (37fea0b)
  2026-02-10 [commit] feat: add TypeScript MCP server (55247d9)
```

Commits with `feat:`, `fix:`, `refactor:`, or `BREAKING CHANGE:` are auto-captured as decisions.

**Best practice:** Use descriptive commit messages to build a decision log:

```bash
git commit -m "feat(auth): switch to jwt rs256

- why: symmetric keys leaked in staging config
- impact: auth service, api gateway, mobile client
- alternative: rotate HMAC keys (rejected — same risk class)"
```

## Health Warnings

ctx-agent detects three categories of codebase risk:

| Warning | Formula | Example |
|---------|---------|---------|
| **Fragile File** | `churn_score > 5.0 AND dependents > 3` | A file changed 20+ times that 5 other files depend on |
| **Large File** | `line_count > 500` | Any file over 500 lines — candidate for splitting |
| **Dead Code** | `commit_count = 0 AND dependents = 0` | Files with no git history and nothing imports them |

```
$ ctx-agent warnings

  ⚠ 2 warnings

  🔥 Fragile files (high churn + many dependents):
    · src/db/mod.rs — 12 changes, 8 dependents (churn: 7.2)

  📏 Large files (>500 lines):
    · src/analyzer/parser.rs — 618 lines (rust)
```

## CLI Reference

```
Usage: ctx-agent [OPTIONS] <COMMAND>

Commands:
  init          Initialize ctx-agent in the current project
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
      --json               Output in JSON format (for agent consumption)
  -h, --help               Print help
  -V, --version            Print version
```

## MCP Server

ctx-agent includes a TypeScript MCP server that exposes all functionality to AI agents via the [Model Context Protocol](https://modelcontextprotocol.io/).

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
      "args": ["/path/to/ctx-agent/mcp-server/dist/index.js"]
    }
  }
}
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `ctx_init` | Initialize ctx-agent in a project |
| `ctx_status` | Project dashboard |
| `ctx_map` | Codebase structure map |
| `ctx_scan` | Incremental re-scan |
| `ctx_query` | Full-text symbol search |
| `ctx_blast_radius` | File impact analysis |
| `ctx_decisions` | Decision history |
| `ctx_learn` | Store knowledge notes |
| `ctx_warnings` | Codebase health warnings |

> **Auto-init:** If a project hasn't been initialized, any MCP tool call will auto-run `ctx-agent init` first. No manual setup needed.

## Architecture

```
ctx-agent/
├── src/
│   ├── main.rs              # CLI (clap) + --json flag
│   ├── lib.rs               # Module exports
│   ├── db/
│   │   ├── mod.rs           # SQLite + FTS5 operations
│   │   ├── models.rs        # Data models (TrackedFile, SymbolKind, etc.)
│   │   └── schema.rs        # Schema migrations
│   ├── analyzer/
│   │   ├── mod.rs           # Orchestrator
│   │   ├── scanner.rs       # File discovery + .gitignore
│   │   ├── parser/
│   │   │   ├── mod.rs       # Parser dispatch
│   │   │   ├── rust.rs      # Rust symbol + import extraction
│   │   │   ├── typescript.rs # TS/JS extraction
│   │   │   └── python.rs    # Python extraction
│   │   └── graph.rs         # Dependency graph + blast radius
│   ├── git/
│   │   └── history.rs       # Commit analysis + churn scoring
│   ├── query/
│   │   ├── search.rs        # FTS5 search
│   │   └── blast.rs         # Blast radius display
│   └── watcher/
│       └── mod.rs           # File watcher daemon
└── mcp-server/
    ├── src/index.ts         # TypeScript MCP server (auto-init)
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

- 🔒 **Local-first** — All data in a single `.ctx/ctx.db` file on your machine
- 📡 **Offline-capable** — No internet, no API keys, no cloud
- ⚡ **Incremental** — File hashes track changes — only re-analyzes what changed
- 🪶 **Zero runtime deps** — Single binary, no Docker, no services
- 🤖 **Agent-native** — Built for MCP, designed for AI agents
- 📤 **Machine-readable** — `--json` flag for programmatic consumption

## License

MIT
