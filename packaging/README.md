# Packaging Files

This directory contains package configurations for various package managers.

## Package Managers

| Directory     | Platform     | Package Manager | Status |
| ------------- | ------------ | --------------- | ------ |
| `homebrew/`   | macOS        | Homebrew        | Ready  |
| `chocolatey/` | Windows      | Chocolatey      | Ready  |
| `scoop/`      | Windows      | Scoop           | Ready  |
| `winget/`     | Windows      | WinGet          | Ready  |
| `aur/`        | Arch Linux   | AUR             | Ready  |
| `snap/`       | Ubuntu/Linux | Snap            | Ready  |

## Usage

### Homebrew (macOS)

```bash
# Build from source formula
brew install --build-from-source ./packaging/homebrew/pleno.rb

# Or from tap (after tap is published)
brew tap otabekoff/tap
brew install pleno
```

### Chocolatey (Windows)

```powershell
# Pack the package
choco pack packaging/chocolatey/pleno.nuspec

# Install locally
choco install pleno -s .

# Push to Chocolatey
choco push pleno.0.1.0.nupkg --source https://push.chocolatey.org/
```

### Scoop (Windows)

```powershell
# Add to bucket (after bucket is published)
scoop bucket add pleno https://github.com/otabekoff/scoop-bucket
scoop install pleno
```

### WinGet (Windows)

Submit to [winget-pkgs](https://github.com/microsoft/winget-pkgs) repository.

### AUR (Arch Linux)

```bash
cd packaging/aur
makepkg -si
```

### Snap (Ubuntu/Linux)

```bash
cd packaging/snap
snapcraft
sudo snap install pleno_*.snap --dangerous
```

## Docker

```bash
# Build image
docker build -t pleno/pleno:latest .

# Run REPL
docker run -it pleno/pleno:latest

# Run a script
docker run -v $(pwd):/workspace pleno/pleno:latest pleno run /workspace/script.pln
```
