# Pleno WASM Toolchain

This crate provides WebAssembly bindings for the Pleno compiler and VM, allowing Pleno code to be executed directly in the browser.

## Features

- **Browser Execution**: Compile and run Pleno code in any modern web browser.
- **Output Capturing**: Captures standard output (print statements) and returns them as a string.
- **Panic Hook**: Integrated with `console_error_panic_hook` for easier debugging.

## Structure

- `src/lib.rs`: Rust bindings using `wasm-bindgen`.
- `www/`: A simple web playground demonstrating the WASM functionality.
  - `index.html`: Playground UI.
  - `style.css`: Premium dark-mode styling.
  - `playground.js`: Logic to load and interact with the WASM module.

## Building

To build the WASM module, you need `wasm-pack` installed:

```bash
# In crates/pleno-wasm
wasm-pack build --target web
```

## Running the Playground

1. Build the WASM module as shown above.
2. Serve the `www` directory using a local web server:

```bash
# Using python
cd www
python3 -m http.server 8080

# Or using any other static server
npx serve .
```

3. Open `http://localhost:8080` in your browser.

## API (JavaScript)

```javascript
import init, { run_source } from "./pkg/pleno_wasm.js";

await init();
const output = run_source("print('Hello from WASM!');");
console.log(output);
```
