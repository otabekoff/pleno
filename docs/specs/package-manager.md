# Package Manager (PLM)

```bash
# Initialize new project
plm init

# Install package
plm install http-client

# Install specific version
plm install json-parser@1.5.0

# Install dev dependency
plm install --dev test-framework

# Update packages
plm update

# Remove package
plm remove http-client

# Run script
plm run build
plm run test
```

```pleno
// package.pln
{
    "name": "my-app",
    "version": "1.0.0",
    "description": "My Pleno application",
    "main": "src/main.pln",
    "scripts": {
        "start": "pleno run src/main.pln",
        "build": "pleno build --output dist/",
        "test": "pleno test tests/"
    },
    "dependencies": {
        "http-client": "^2.1.0",
        "json-parser": "^1.5.0"
    },
    "devDependencies": {
        "test-framework": "^3.0.0"
    }
}
```