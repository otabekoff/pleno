# Distribution Strategy

Pleno will ship via a staged distribution plan that balances stability and
adoption speed. Start with official binaries and an install script, then add
platform package managers and ecosystem tooling.

## Phases

### Phase 1: Development and Early Adopters
- GitHub releases with binaries
- Official install script
- Cargo install (Rust users)
- Basic docs site

### Phase 2: Public Release
- Homebrew (macOS)
- Chocolatey (Windows)
- Docker images
- VS Code extension
- PLM package registry (basic)

### Phase 3: Ecosystem Growth
- Debian/Ubuntu packages
- Snap/Flatpak
- AUR (Arch)
- Expanded PLM registry with search

## Installation Methods

### Standalone Installers
- Windows: MSI, WinGet, Chocolatey, Scoop
- macOS: .pkg, Homebrew, MacPorts
- Linux: .deb, .rpm, AUR, Snap, Flatpak, AppImage

### Install Script

- `install.sh` for Unix/macOS
- `install.ps1` for Windows
- Installs to `~/.pleno/bin` and updates PATH

### Version Manager

- `plenovm` (like `nvm` or `rbenv`)
- Supports install/use/list for multiple versions

## Binary Layout

```
pleno-<version>-<platform>/
├── pleno       (Main CLI)
├── plm         (Package Manager)
├── pleno-lsp   (Language Server)
├── pleno-repl  (Interactive REPL)
├── LICENSE
└── README.md
```

## CI/CD Release Automation

- Build per target on tag push
- Create release assets with checksums
- Publish Docker images
- Optional crates.io publish

## Release Channels

- Stable: default, every 6-8 weeks
- Beta: pre-release
- Nightly: daily builds

## Distribution Checklist

Pre-release:
- Tests passing
- Docs updated
- Changelog updated
- Version bumped
- Benchmarks run

Release assets:
- Source tarball
- Linux/macOS/Windows binaries
- Docker images
- Checksums and signatures

## Recommended Priority

1. GitHub releases with binaries
2. Install script
3. Cargo install
4. Docker image
5. Homebrew + Chocolatey
6. VS Code extension
7. PLM registry
