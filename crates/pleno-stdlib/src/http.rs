use pleno_value::Value;
use std::collections::HashMap;
use std::rc::Rc;

pub fn http_get(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(pleno_value::RuntimeError::new("get() expects URL string")),
    };
    let response = reqwest::blocking::get(url)
        .map_err(|err| pleno_value::RuntimeError::new(format!("HTTP request failed: {err}")))?;
    let body = response.text().map_err(|err| {
        pleno_value::RuntimeError::new(format!("failed to read response body: {err}"))
    })?;
    Ok(Value::String(Rc::new(body)))
}

pub fn http_post(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(pleno_value::RuntimeError::new("post() expects URL string")),
    };
    let body = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(url)
        .body(body.to_string())
        .send()
        .map_err(|err| pleno_value::RuntimeError::new(format!("HTTP request failed: {err}")))?;
    let body = response.text().map_err(|err| {
        pleno_value::RuntimeError::new(format!("failed to read response body: {err}"))
    })?;
    Ok(Value::String(Rc::new(body)))
}

pub fn http_put(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(pleno_value::RuntimeError::new("put() expects URL string")),
    };
    let body = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    let client = reqwest::blocking::Client::new();
    let response = client
        .put(url)
        .body(body.to_string())
        .send()
        .map_err(|err| pleno_value::RuntimeError::new(format!("HTTP request failed: {err}")))?;
    let body = response.text().map_err(|err| {
        pleno_value::RuntimeError::new(format!("failed to read response body: {err}"))
    })?;
    Ok(Value::String(Rc::new(body)))
}

pub fn http_delete(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "delete() expects URL string",
            ));
        }
    };
    let client = reqwest::blocking::Client::new();
    let response = client
        .delete(url)
        .send()
        .map_err(|err| pleno_value::RuntimeError::new(format!("HTTP request failed: {err}")))?;
    let body = response.text().map_err(|err| {
        pleno_value::RuntimeError::new(format!("failed to read response body: {err}"))
    })?;
    Ok(Value::String(Rc::new(body)))
}
// --- Router Implementation ---

pub fn http_router(
    vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let gc = vm.get_gc();
    let mut router = HashMap::new();

    router.insert("get".to_string(), Value::NativeFunction(http_router_get));
    router.insert("post".to_string(), Value::NativeFunction(http_router_post));

    Ok(Value::Map(gc.allocate(pleno_value::MapState::new(router))))
}

pub fn http_router_get(
    _vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    // Stub implementation: just print registering
    // In real impl, store handler
    println!("Router: Registered GET handler");
    Ok(Value::Null)
}

pub fn http_router_post(
    _vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    // Stub implementation
    println!("Router: Registered POST handler");
    Ok(Value::Null)
}

// --- Server Implementation ---

pub fn http_server(
    vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let gc = vm.get_gc();
    let mut server = HashMap::new();

    server.insert(
        "listen".to_string(),
        Value::NativeFunction(http_server_listen),
    );

    Ok(Value::Map(gc.allocate(pleno_value::MapState::new(server))))
}

pub fn http_server_listen(
    _vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if let Some(port) = args.first() {
        println!("Server mock listening on port {}", port);
    }
    // Execute callback if present
    if let Some(Value::Function(_func)) = args.get(1) {
        // Need way to call function from native...
        // For now, just ignore or print
        println!("(Server listen callback triggered)");
    }
    Ok(Value::Null)
}
