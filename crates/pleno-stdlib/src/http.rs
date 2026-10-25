use indexmap::IndexMap;
use pleno_value::{
    BoundNativeMethodState, MapState, NativeFunction, NativeResource, RuntimeError, VMProvider,
    Value,
};
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
    methods_map.insert(
        "PATCH".to_string(),
        Value::Map(gc.allocate(MapState::new(IndexMap::new()))?),
    );

    let routes_storage = Value::Map(gc.allocate(MapState::new(methods_map))?);

    let mut router = IndexMap::new();
    router.insert("_routes".to_string(), routes_storage);

    // ...
    let router_val = Value::Map(gc.allocate(MapState::new(router))?);

    if let Value::Map(ref map_ptr) = router_val {
        let mut map = map_ptr.borrow_mut();
        let bind = |vm: &mut dyn VMProvider, func: NativeFunction| {
            Value::BoundNativeMethod(
                vm.get_gc()
                    .allocate(BoundNativeMethodState {
                        receiver: router_val.clone(),
                        function: func,
                    })
                    .unwrap(),
            )
        };
        map.entries.insert("get".to_string(), bind(vm, router_get));
        map.entries
            .insert("post".to_string(), bind(vm, router_post));
        map.entries.insert("put".to_string(), bind(vm, router_put));
        map.entries
            .insert("delete".to_string(), bind(vm, router_delete));
        map.entries
            .insert("patch".to_string(), bind(vm, router_patch));
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
fn router_patch(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    add_route(vm, args, "PATCH")
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
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: server_val.clone(),
                function: http_server_listen,
            })?),
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
                    let on_accept = {
                        let gc = vm.get_gc();
                        Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                            receiver: this_server.clone(),
                            function: http_server_on_accept,
                        })?)
                    };
                    vm.register_io(l, mio::Interest::READABLE, on_accept, true)?;
                }
            }
        }
    }

    Ok(Value::Null)
}

#[allow(clippy::collapsible_if)]
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
        if let Some(inner) = state.inner.get_mut().as_mut() {
            if let Some(listener) = inner.downcast_mut::<mio::net::TcpListener>() {
                while let Ok((stream, _)) = listener.accept() {
                    handle_request(vm, stream, router_ref.as_ref())?;
                }
            }
        }
    }

    Ok(Value::Null)
}

/// Request body size limit, to keep a single client from exhausting memory.
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

struct HttpRequest {
    method: String,
    path: String,
    query: IndexMap<String, String>,
    headers: IndexMap<String, String>,
    body: String,
}

/// Reads one HTTP/1.1 request; `None` if the client sent nothing usable.
fn read_request(stream: &mut std::net::TcpStream) -> Option<HttpRequest> {
    use std::io::Read;
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_uppercase();
    let target = parts.next()?;

    let mut headers = IndexMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_lowercase(), value.trim().to_string());
        }
    }

    let length = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .min(MAX_BODY_BYTES);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;

    let (path, query_string) = target.split_once('?').unwrap_or((target, ""));
    let query = query_string
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (key.to_string(), value.to_string())
        })
        .collect();

    Some(HttpRequest {
        method,
        path: path.to_string(),
        query,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// Matches a route pattern such as `/users/:id` against a request path,
/// returning the captured parameters.
fn match_route(pattern: &str, path: &str) -> Option<IndexMap<String, String>> {
    let pattern_parts: Vec<&str> = pattern.trim_end_matches('/').split('/').collect();
    let path_parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    if pattern_parts.len() != path_parts.len() {
        return None;
    }
    let mut params = IndexMap::new();
    for (expected, actual) in pattern_parts.iter().zip(&path_parts) {
        if let Some(name) = expected.strip_prefix(':') {
            params.insert(name.to_string(), actual.to_string());
        } else if expected != actual {
            return None;
        }
    }
    Some(params)
}

fn string_map(
    vm: &mut dyn VMProvider,
    entries: &IndexMap<String, String>,
) -> Result<Value, RuntimeError> {
    let map = entries
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(Rc::new(v.clone()))))
        .collect();
    Ok(Value::Map(vm.get_gc().allocate(MapState::new(map))?))
}

/// The `res` object passed to handlers: `status(code)`, `header(name, value)`,
/// `send(body)` and `json(value)`. `status` and `header` return `res` for chaining.
fn new_response(vm: &mut dyn VMProvider) -> Result<Value, RuntimeError> {
    let headers = Value::Map(vm.get_gc().allocate(MapState::new(IndexMap::new()))?);
    let mut entries = IndexMap::new();
    entries.insert("statusCode".to_string(), Value::Int(200));
    entries.insert("body".to_string(), Value::Null);
    entries.insert("headers".to_string(), headers);
    let res = Value::Map(vm.get_gc().allocate(MapState::new(entries))?);
    for (name, function) in [
        ("status", response_status as NativeFunction),
        ("header", response_header),
        ("send", response_send),
        ("json", response_json),
    ] {
        let method = Value::BoundNativeMethod(vm.get_gc().allocate(BoundNativeMethodState {
            receiver: res.clone(),
            function,
        })?);
        if let Value::Map(m) = &res {
            m.borrow_mut().entries.insert(name.to_string(), method);
        }
    }
    Ok(res)
}

fn response_map(args: &[Value]) -> Result<&pleno_value::gc::GcRc<MapState>, RuntimeError> {
    match args.first() {
        Some(Value::Map(m)) => Ok(m),
        _ => Err(RuntimeError::new("expected response object")),
    }
}

fn response_status(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let code = match args.get(1) {
        Some(Value::Int(code)) => *code,
        _ => return Err(RuntimeError::new("res.status(code) expects an integer")),
    };
    response_map(args)?
        .borrow_mut()
        .entries
        .insert("statusCode".to_string(), Value::Int(code));
    Ok(args[0].clone())
}

fn response_header(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let (Some(Value::String(name)), Some(value)) = (args.get(1), args.get(2)) else {
        return Err(RuntimeError::new(
            "res.header(name, value) expects a name and value",
        ));
    };
    let headers = response_map(args)?.borrow().entries.get("headers").cloned();
    if let Some(Value::Map(headers)) = headers {
        headers
            .borrow_mut()
            .entries
            .insert(name.to_string(), Value::String(Rc::new(value.to_string())));
    }
    Ok(args[0].clone())
}

fn response_send(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let body = args.get(1).map(|v| v.to_string()).unwrap_or_default();
    response_map(args)?
        .borrow_mut()
        .entries
        .insert("body".to_string(), Value::String(Rc::new(body)));
    Ok(args[0].clone())
}

fn response_json(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let json = crate::json::value_to_json_string(args.get(1).unwrap_or(&Value::Null))?;
    let content_type = [
        args[0].clone(),
        Value::String(Rc::new("Content-Type".to_string())),
        Value::String(Rc::new("application/json".to_string())),
    ];
    response_header(vm, &content_type)?;
    response_send(vm, &[args[0].clone(), Value::String(Rc::new(json))])
}

fn reason_phrase(status: i64) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "",
    }
}

/// Writes a response. For a `HEAD` request (`head_only`) the headers,
/// `Content-Length` included, are those of the full response, but no body.
fn write_response(
    stream: &mut std::net::TcpStream,
    status: i64,
    headers: &[(String, String)],
    body: &str,
    head_only: bool,
) {
    use std::io::Write;
    let mut response = format!("HTTP/1.1 {} {}\r\n", status, reason_phrase(status));
    let has_content_type = headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"));
    if !has_content_type {
        response.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    }
    for (name, value) in headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    response.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    if !head_only {
        response.push_str(body);
    }
    stream.write_all(response.as_bytes()).ok();
    stream.flush().ok();
}

fn handle_request(
    vm: &mut dyn VMProvider,
    stream: mio::net::TcpStream,
    router_ref: Option<&Value>,
) -> Result<(), RuntimeError> {
    // Serve each connection with blocking I/O; the listener stays on the event loop.
    let mut stream: std::net::TcpStream = stream.into();
    stream.set_nonblocking(false).ok();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();

    let Some(request) = read_request(&mut stream) else {
        return Ok(());
    };

    // Find the first route for this method whose pattern matches the path.
    // A HEAD request is served by the `get` routes, without the body.
    let head_only = request.method == "HEAD";
    let route_method = if head_only {
        "GET"
    } else {
        request.method.as_str()
    };
    let mut matched = None;
    if let Some(Value::Map(router)) = router_ref
        && let Some(Value::Map(routes_store)) = router.borrow().entries.get("_routes")
        && let Some(Value::Map(method_routes)) = routes_store.borrow().entries.get(route_method)
    {
        for (pattern, handler) in &method_routes.borrow().entries {
            if let Some(params) = match_route(pattern, &request.path) {
                matched = Some((handler.clone(), params));
                break;
            }
        }
    }

    let Some((handler, params)) = matched else {
        write_response(&mut stream, 404, &[], "Not Found", head_only);
        return Ok(());
    };

    let mut req_data = IndexMap::new();
    req_data.insert(
        "method".to_string(),
        Value::String(Rc::new(request.method.clone())),
    );
    req_data.insert(
        "path".to_string(),
        Value::String(Rc::new(request.path.clone())),
    );
    req_data.insert("params".to_string(), string_map(vm, &params)?);
    req_data.insert("query".to_string(), string_map(vm, &request.query)?);
    req_data.insert("headers".to_string(), string_map(vm, &request.headers)?);
    req_data.insert("body".to_string(), Value::String(Rc::new(request.body)));
    let req = Value::Map(vm.get_gc().allocate(MapState::new(req_data))?);
    let res = new_response(vm)?;

    match vm.call(handler, vec![req, res.clone()]) {
        // An async handler returns a promise: respond once it settles.
        Ok(Value::Map(promise))
            if promise.borrow().get("variant").and_then(|v| v.as_str()) == Some("Pending") =>
        {
            let mut context = IndexMap::new();
            let stream = NativeResource::new("http_stream".to_string(), Box::new(stream));
            context.insert(
                "stream".to_string(),
                Value::Resource(vm.get_gc().allocate(stream)?),
            );
            context.insert("res".to_string(), res);
            context.insert("headOnly".to_string(), Value::Bool(head_only));
            context.insert(
                "request".to_string(),
                Value::String(Rc::new(format!("{} {}", request.method, request.path))),
            );
            let context = Value::Map(vm.get_gc().allocate(MapState::new(context))?);
            let bind = |vm: &mut dyn VMProvider, function: NativeFunction| {
                vm.get_gc()
                    .allocate(BoundNativeMethodState {
                        receiver: context.clone(),
                        function,
                    })
                    .map(Value::BoundNativeMethod)
            };
            let on_fulfilled = bind(vm, finish_async_response)?;
            let on_rejected = bind(vm, fail_async_response)?;
            crate::promise::register_callback(
                vm,
                &Value::Map(promise),
                on_fulfilled,
                on_rejected,
                0,
            )?;
        }
        Ok(returned) => send_handler_response(&mut stream, &res, &returned, head_only),
        Err(e) => send_handler_error(
            &mut stream,
            &format!("{} {}", request.method, request.path),
            &e.message,
            head_only,
        ),
    }
    Ok(())
}

/// Writes the response a handler built on `res` (or the body it returned).
fn send_handler_response(
    stream: &mut std::net::TcpStream,
    res: &Value,
    returned: &Value,
    head_only: bool,
) {
    let Value::Map(res_map) = res else {
        return;
    };
    let res_map = res_map.borrow();
    let status = res_map
        .entries
        .get("statusCode")
        .and_then(|v| v.as_int())
        .unwrap_or(200);
    let headers: Vec<(String, String)> = match res_map.entries.get("headers") {
        Some(Value::Map(h)) => h
            .borrow()
            .entries
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect(),
        _ => Vec::new(),
    };
    // A handler may also just return its body instead of calling `res.send`.
    let body = match (res_map.entries.get("body"), returned) {
        (Some(Value::String(body)), _) => body.to_string(),
        (_, Value::String(body)) => body.to_string(),
        _ => String::new(),
    };
    write_response(stream, status, &headers, &body, head_only);
}

fn send_handler_error(
    stream: &mut std::net::TcpStream,
    request: &str,
    message: &str,
    head_only: bool,
) {
    let message = message
        .strip_prefix("uncaught exception: ")
        .unwrap_or(message);
    eprintln!("{request}: {message}");
    write_response(
        stream,
        500,
        &[],
        &format!("Internal Server Error: {message}"),
        head_only,
    );
}

/// Takes the connection and request details stored for an async handler.
fn async_context(args: &[Value]) -> Option<(std::net::TcpStream, Value, String, bool)> {
    let Some(Value::Map(context)) = args.first() else {
        return None;
    };
    let context = context.borrow();
    let res = context.get("res").cloned()?;
    let request = context
        .get("request")
        .map(|v| v.to_string())
        .unwrap_or_default();
    let head_only = matches!(context.get("headOnly"), Some(Value::Bool(true)));
    let Some(Value::Resource(resource)) = context.get("stream") else {
        return None;
    };
    let stream = resource.borrow().inner.borrow_mut().take()?;
    let stream = stream.downcast::<std::net::TcpStream>().ok()?;
    Some((*stream, res, request, head_only))
}

fn finish_async_response(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if let Some((mut stream, res, _, head_only)) = async_context(args) {
        let returned = args.get(1).unwrap_or(&Value::Null);
        send_handler_response(&mut stream, &res, returned, head_only);
    }
    Ok(Value::Null)
}

fn fail_async_response(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if let Some((mut stream, _, request, head_only)) = async_context(args) {
        let reason = args.get(1).map(|v| v.to_string()).unwrap_or_default();
        send_handler_error(&mut stream, &request, &reason, head_only);
    }
    Ok(Value::Null)
}
