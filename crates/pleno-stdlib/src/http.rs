use indexmap::IndexMap;
use pleno_value::{MapState, NativeFunction, NativeResource, RuntimeError, VMProvider, Value};
use std::io::{BufRead, BufReader};
use std::rc::Rc;

// --- Client Methods ---

pub fn http_get(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("get() expects URL string")),
    };

    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| RuntimeError::new(format!("Client error: {}", e)))?;
    let response = client
        .get(url)
        .send()
        .map_err(|err| RuntimeError::new(format!("Request failed: {}", err)))?;
    let body = response
        .text()
        .map_err(|err| RuntimeError::new(format!("Body error: {}", err)))?;
    Ok(Value::String(Rc::new(body)))
}

pub fn http_post(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("post() expects URL string")),
    };
    let body = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };

    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| RuntimeError::new(format!("Client error: {}", e)))?;
    let response = client
        .post(url)
        .body(body.to_string())
        .send()
        .map_err(|err| RuntimeError::new(format!("Request failed: {}", err)))?;
    let body = response
        .text()
        .map_err(|err| RuntimeError::new(format!("Body error: {}", err)))?;
    Ok(Value::String(Rc::new(body)))
}

pub fn http_put(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("put() expects URL string")),
    };
    let body = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };

    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| RuntimeError::new(format!("Client error: {}", e)))?;
    let response = client
        .put(url)
        .body(body.to_string())
        .send()
        .map_err(|err| RuntimeError::new(format!("Request failed: {}", err)))?;
    Ok(Value::String(Rc::new(response.text().map_err(|e| {
        RuntimeError::new(format!("Body error: {}", e))
    })?)))
}

pub fn http_delete(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("delete() expects URL string")),
    };

    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| RuntimeError::new(format!("Client error: {}", e)))?;
    let response = client
        .delete(url)
        .send()
        .map_err(|err| RuntimeError::new(format!("Request failed: {}", err)))?;
    Ok(Value::String(Rc::new(response.text().map_err(|e| {
        RuntimeError::new(format!("Body error: {}", e))
    })?)))
}

pub fn http_patch(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("patch() expects URL string")),
    };
    let body = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };

    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| RuntimeError::new(format!("Client error: {}", e)))?;
    let response = client
        .patch(url)
        .body(body.to_string())
        .send()
        .map_err(|err| RuntimeError::new(format!("Request failed: {}", err)))?;
    Ok(Value::String(Rc::new(response.text().map_err(|e| {
        RuntimeError::new(format!("Body error: {}", e))
    })?)))
}

pub fn http_head(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("head() expects URL string")),
    };

    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| RuntimeError::new(format!("Client error: {}", e)))?;
    let response = client
        .head(url)
        .send()
        .map_err(|err| RuntimeError::new(format!("Request failed: {}", err)))?;

    let gc = vm.get_gc();
    let mut headers_map = IndexMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            headers_map.insert(key.to_string(), Value::String(Rc::new(v.to_string())));
        }
    }
    headers_map.insert(
        "status".to_string(),
        Value::Int(response.status().as_u16() as i64),
    );
    Ok(Value::Map(gc.allocate(MapState::new(headers_map))?))
}

pub fn http_options(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("options() expects URL string")),
    };

    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| RuntimeError::new(format!("Client error: {}", e)))?;
    let response = client
        .request(reqwest::Method::OPTIONS, url)
        .send()
        .map_err(|err| RuntimeError::new(format!("Request failed: {}", err)))?;

    let gc = vm.get_gc();
    let mut headers_map = IndexMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            headers_map.insert(key.to_string(), Value::String(Rc::new(v.to_string())));
        }
    }
    headers_map.insert(
        "status".to_string(),
        Value::Int(response.status().as_u16() as i64),
    );
    Ok(Value::Map(gc.allocate(MapState::new(headers_map))?))
}

pub fn http_request(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }

    let method_str = match args.first() {
        Some(Value::String(s)) => s.as_str().to_uppercase(),
        _ => return Err(RuntimeError::new("request() expects method")),
    };
    let url = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("request() expects URL")),
    };
    let body = match args.get(2) {
        Some(Value::String(s)) => Some(s.as_str().to_string()),
        _ => None,
    };

    let method = match method_str.as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "PATCH" => reqwest::Method::PATCH,
        "DELETE" => reqwest::Method::DELETE,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        other => reqwest::Method::from_bytes(other.as_bytes())
            .map_err(|_| RuntimeError::new("Invalid method"))?,
    };

    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| RuntimeError::new(format!("Client error: {}", e)))?;
    let mut request = client.request(method, url);
    if let Some(b) = body {
        request = request.body(b);
    }

    if let Some(Value::Map(headers_ref)) = args.get(3) {
        let headers = headers_ref.borrow();
        for (key, value) in headers.entries.iter() {
            if let Value::String(v) = value {
                request = request.header(key.as_str(), v.as_str());
            }
        }
    }

    let response = request
        .send()
        .map_err(|err| RuntimeError::new(format!("Request failed: {}", err)))?;

    let gc = vm.get_gc();
    let mut result = IndexMap::new();
    result.insert(
        "status".to_string(),
        Value::Int(response.status().as_u16() as i64),
    );
    result.insert(
        "ok".to_string(),
        Value::Bool(response.status().is_success()),
    );

    let mut headers_map = IndexMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            headers_map.insert(key.to_string(), Value::String(Rc::new(v.to_string())));
        }
    }
    result.insert(
        "headers".to_string(),
        Value::Map(gc.allocate(MapState::new(headers_map))?),
    );

    let body_text = response.text().unwrap_or_default();
    result.insert("body".to_string(), Value::String(Rc::new(body_text)));

    Ok(Value::Map(gc.allocate(MapState::new(result))?))
}

// --- Router ---

pub fn http_router(vm: &mut dyn VMProvider, _args: &[Value]) -> Result<Value, RuntimeError> {
    let gc = vm.get_gc();
    // Internal routes: Map<Method, Map<Path, Handler>>
    // Or just Map<Path, Map<Method, Handler>>? Pleno doesn't have nested native logic easily.
    // Let's store: _routes -> Map<"GET:/path", Handler> ? Simple flat map.
    // Or _routes -> Map<"GET", Map<"/", Handler>>
    let mut methods_map = IndexMap::new();
    methods_map.insert(
        "GET".to_string(),
        Value::Map(gc.allocate(MapState::new(IndexMap::new()))?),
    );
    methods_map.insert(
        "POST".to_string(),
        Value::Map(gc.allocate(MapState::new(IndexMap::new()))?),
    );
    methods_map.insert(
        "PUT".to_string(),
        Value::Map(gc.allocate(MapState::new(IndexMap::new()))?),
    );
    methods_map.insert(
        "DELETE".to_string(),
        Value::Map(gc.allocate(MapState::new(IndexMap::new()))?),
    );

    let routes_storage = Value::Map(gc.allocate(MapState::new(methods_map))?);

    let mut router = IndexMap::new();
    router.insert("_routes".to_string(), routes_storage);

    // ...
    let router_val = Value::Map(gc.allocate(MapState::new(router))?);

    if let Value::Map(ref map_ptr) = router_val {
        let mut map = map_ptr.borrow_mut();
        let bind = |func: NativeFunction| Value::BoundNativeMethod {
            receiver: Box::new(router_val.clone()),
            function: func,
        };
        map.entries.insert("get".to_string(), bind(router_get));
        map.entries.insert("post".to_string(), bind(router_post));
        map.entries.insert("put".to_string(), bind(router_put));
        map.entries
            .insert("delete".to_string(), bind(router_delete));
    }

    Ok(router_val)
}

fn add_route(
    _vm: &mut dyn VMProvider,
    args: &[Value],
    method: &str,
) -> Result<Value, RuntimeError> {
    // args[0] = this (Router), args[1] = path, args[2] = handler
    if args.len() < 3 {
        return Err(RuntimeError::new(format!(
            "Router.{} expects path and handler",
            method.to_lowercase()
        )));
    }

    let this_router = &args[0];
    let path = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Err(RuntimeError::new("Path must be string")),
    };
    let handler = args[2].clone();

    let routes_ref = match this_router {
        Value::Map(m) => {
            let b = m.borrow();
            b.entries.get("_routes").cloned()
        }
        _ => return Err(RuntimeError::new("Invalid Router object")),
    };

    if let Some(Value::Map(routes_map)) = routes_ref {
        let routes = routes_map.borrow();
        if let Some(Value::Map(method_routes)) = routes.entries.get(method) {
            method_routes
                .borrow_mut()
                .entries
                .insert(path.to_string(), handler);
        }
    }

    Ok(this_router.clone()) // Fluent API
}

fn router_get(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(vm, args, "GET")
}
fn router_post(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(vm, args, "POST")
}
fn router_put(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(vm, args, "PUT")
}
fn router_delete(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(vm, args, "DELETE")
}

// --- Server ---

pub fn http_server(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let router = match args.first() {
        Some(val) => val.clone(),
        None => return Err(RuntimeError::new("Server(router) requires a router")),
    };

    let gc = vm.get_gc();
    let mut server = IndexMap::new();
    server.insert("_router".to_string(), router);

    let server_val = Value::Map(gc.allocate(MapState::new(server))?);

    if let Value::Map(ref map_ptr) = server_val {
        let mut map = map_ptr.borrow_mut();
        map.entries.insert(
            "listen".to_string(),
            Value::BoundNativeMethod {
                receiver: Box::new(server_val.clone()),
                function: http_server_listen,
            },
        );
    }
    Ok(server_val)
}

#[allow(clippy::collapsible_if)]
pub fn http_server_listen(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // args[0] = this (Server), args[1] = port, args[2] = callback (optional)
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }

    let this_server = &args[0];
    let port = match args.get(1) {
        Some(Value::Int(p)) => *p as u16,
        Some(Value::Float(p)) => *p as u16,
        _ => return Err(RuntimeError::new("listen() expects port")),
    };

    let addr = format!("0.0.0.0:{}", port)
        .parse()
        .map_err(|e| RuntimeError::new(format!("Invalid address: {}", e)))?;
    let listener = mio::net::TcpListener::bind(addr)
        .map_err(|e| RuntimeError::new(format!("Bind failed: {}", e)))?;

    println!("Server listening on http://0.0.0.0:{}", port);

    // Call callback if provided
    if let Some(cb) = args.get(2) {
        if let Value::Function(_) | Value::Closure(_) | Value::NativeFunction(_) = cb {
            vm.call(cb.clone(), vec![])?;
        }
    }

    // Store listener to keep it alive
    let gc = vm.get_gc();
    let listener_res = Value::Resource(gc.allocate(NativeResource::new(
        "http_listener".to_string(),
        Box::new(listener),
    ))?);

    if let Value::Map(m) = this_server {
        m.borrow_mut()
            .entries
            .insert("_listener".to_string(), listener_res);
    }

    // Register with event loop
    // Since we need to borrow the listener to register it, we do it now.
    // The listener_res in the map will keep it alive.

    // We retrieve it back because we need a &mut reference for register_io
    if let Value::Map(m) = this_server {
        let listener_val = m.borrow().entries.get("_listener").cloned();
        if let Some(Value::Resource(r)) = listener_val {
            let mut state = r.borrow_mut();
            if let Some(inner) = state.inner.get_mut().as_mut() {
                if let Some(l) = inner.downcast_mut::<mio::net::TcpListener>() {
                    vm.register_io(
                        l,
                        mio::Interest::READABLE,
                        Value::BoundNativeMethod {
                            receiver: Box::new(this_server.clone()),
                            function: http_server_on_accept,
                        },
                        true,
                    )?;
                }
            }
        }
    }

    Ok(Value::Null)
}

pub fn http_server_on_accept(
    vm: &mut dyn VMProvider,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    // args[0] = this (Server)
    let this_server = &args[0];
    let listener_val = match this_server {
        Value::Map(m) => m.borrow().entries.get("_listener").cloned(),
        _ => return Err(RuntimeError::new("on_accept: Invalid Server")),
    };

    let router_ref = match this_server {
        Value::Map(m) => m.borrow().entries.get("_router").cloned(),
        _ => return Err(RuntimeError::new("on_accept: Missing router")),
    };

    if let Some(Value::Resource(r)) = listener_val {
        let mut state = r.borrow_mut();
        if let Some(inner) = state.inner.get_mut().as_mut()
            && let Some(listener) = inner.downcast_mut::<mio::net::TcpListener>()
        {
            while let Ok((mut stream, _)) = listener.accept() {
                handle_request(vm, &mut stream, router_ref.as_ref())?;
            }
        }
    }

    Ok(Value::Null)
}

fn handle_request(
    vm: &mut dyn VMProvider,
    stream: &mut mio::net::TcpStream,
    router_ref: Option<&Value>,
) -> Result<(), RuntimeError> {
    use std::io::Write;
    let mut reader = BufReader::new(&mut *stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
        return Ok(());
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let method = parts[0];
    let path = parts[1];

    // Headers lookup (simplified)
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
    }

    // Dispatch to Router
    let mut handler = None;
    if let Some(Value::Map(router)) = router_ref
        && let Some(Value::Map(routes_store)) = router.borrow().entries.get("_routes")
        && let Some(Value::Map(method_routes)) = routes_store.borrow().entries.get(method)
    {
        handler = method_routes.borrow().entries.get(path).cloned();
    }

    let response = if let Some(h) = handler {
        let gc = vm.get_gc();
        let mut req_data = IndexMap::new();
        req_data.insert(
            "method".to_string(),
            Value::String(Rc::new(method.to_string())),
        );
        req_data.insert("path".to_string(), Value::String(Rc::new(path.to_string())));
        let req = Value::Map(gc.allocate(MapState::new(req_data)).unwrap());

        let res_data = IndexMap::new();
        let res = Value::Map(gc.allocate(MapState::new(res_data)).unwrap());

        match vm.call(h, vec![req, res]) {
            Ok(val) => {
                if let Value::Map(m) = val {
                    let b = m.borrow();
                    let body = b
                        .entries
                        .get("body")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let status = b
                        .entries
                        .get("status")
                        .and_then(|v| v.as_int())
                        .unwrap_or(200);
                    format!("HTTP/1.1 {} OK\r\n\r\n{}", status, body)
                } else if let Value::String(s) = val {
                    format!("HTTP/1.1 200 OK\r\n\r\n{}", s.as_str())
                } else {
                    "HTTP/1.1 200 OK\r\n\r\n".to_string()
                }
            }
            Err(e) => format!(
                "HTTP/1.1 500 Internal Server Error\r\n\r\nError: {}",
                e.message
            ),
        }
    } else {
        "HTTP/1.1 404 Not Found\r\n\r\nNot Found".to_string()
    };

    stream.write_all(response.as_bytes()).ok();
    Ok(())
}
