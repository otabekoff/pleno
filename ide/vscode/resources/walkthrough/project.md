# Create your first Pleno Project

Pleno projects use a `pleno.toml` file to manage dependencies and scripts.

### Initialize a new project

1. Create a new directory for your project.
2. Open it in VS Code.
3. Run **Pleno: Create New Project**, or in the terminal:
   ```bash
   plm init my-awesome-project
   ```

### Project Structure

- `pleno.toml`: Project manifest (TOML format) with dependencies and scripts.
- `src/main.pln`: The entry point.
- `tests/`: Tests, run with `plm test` or the Test Explorer.

Once created, you should see the **Pleno Project Explorer** appearing in the sidebar!

[Open Project Explorer](command:workbench.view.extension.pleno-explorer)
