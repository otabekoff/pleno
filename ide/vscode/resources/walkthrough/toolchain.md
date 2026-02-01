# Setup the Pleno Toolchain

To use the full power of Pleno, you need the toolchain installed on your system.

### Installation

If you haven't installed it yet, run the following command in your terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh
```

### Verification

Once installed, verify it by running:

```bash
pleno --version
```

The extension will automatically detect the toolchain in your PATH. If you are developing Pleno itself, it will fall back to your local build artifacts.

[Check Toolchain Status](command:pleno.showOutput)
