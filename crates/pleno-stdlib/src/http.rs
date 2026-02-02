use indexmap::IndexMap;
use pleno_value::Value;
use std::rc::Rc;

pub fn http_get(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Network access denied",
        ));
    }
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
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Network access denied",
        ));
    }
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
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Network access denied",
        ));
    }
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
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Network access denied",
        ));
    }
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

pub fn http_patch(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Network access denied",
        ));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(pleno_value::RuntimeError::new("patch() expects URL string")),
    };
    let body = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    let client = reqwest::blocking::Client::new();
    let response = client
        .patch(url)
        .body(body.to_string())
        .send()
        .map_err(|err| pleno_value::RuntimeError::new(format!("HTTP request failed: {err}")))?;
    let body = response.text().map_err(|err| {
        pleno_value::RuntimeError::new(format!("failed to read response body: {err}"))
    })?;
    Ok(Value::String(Rc::new(body)))
}

pub fn http_head(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Network access denied",
        ));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(pleno_value::RuntimeError::new("head() expects URL string")),
    };
    let client = reqwest::blocking::Client::new();
    let response = client
        .head(url)
        .send()
        .map_err(|err| pleno_value::RuntimeError::new(format!("HTTP request failed: {err}")))?;

    // Return headers as a map
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
    Ok(Value::Map(
        gc.allocate(pleno_value::MapState::new(headers_map))?,
    ))
}

pub fn http_options(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Network access denied",
        ));
    }
    let url = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "options() expects URL string",
            ));
        }
    };
    let client = reqwest::blocking::Client::new();
    let request = client.request(reqwest::Method::OPTIONS, url);
    let response = request
        .send()
        .map_err(|err| pleno_value::RuntimeError::new(format!("HTTP request failed: {err}")))?;

    // Return headers (Allow header is key for OPTIONS)
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
    Ok(Value::Map(
        gc.allocate(pleno_value::MapState::new(headers_map))?,
    ))
}

/// Generic request method supporting any HTTP method
/// Usage: request(method, url, body?, headers?)
pub fn http_request(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Network access denied",
        ));
    }

    let method_str = match args.first() {
        Some(Value::String(s)) => s.as_str().to_uppercase(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "request() expects method string as first arg",
            ));
        }
    };

    let url = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "request() expects URL string as second arg",
            ));
        }
    };

    let body = match args.get(2) {
        Some(Value::String(s)) => Some(s.as_str().to_string()),
        Some(Value::Null) => None,
        None => None,
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "request() body must be string or null",
            ));
        }
    };

    let method = match method_str.as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "PATCH" => reqwest::Method::PATCH,
        "DELETE" => reqwest::Method::DELETE,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        "TRACE" => reqwest::Method::TRACE,
        "CONNECT" => reqwest::Method::CONNECT,
        other => {
            // Try custom method for WebDAV etc
            reqwest::Method::from_bytes(other.as_bytes()).map_err(|_| {
                pleno_value::RuntimeError::new(format!("Invalid HTTP method: {other}"))
            })?
        }
    };

    let client = reqwest::blocking::Client::new();
    let mut request = client.request(method, url);

    if let Some(b) = body {
        request = request.body(b);
    }

    // Add custom headers if provided
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
        .map_err(|err| pleno_value::RuntimeError::new(format!("HTTP request failed: {err}")))?;

    // Return full response object
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
        Value::Map(gc.allocate(pleno_value::MapState::new(headers_map))?),
    );

    let body_text = response.text().unwrap_or_default();
    result.insert("body".to_string(), Value::String(Rc::new(body_text)));

    Ok(Value::Map(gc.allocate(pleno_value::MapState::new(result))?))
}

// --- Router Implementation ---

pub fn http_router(
    vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let gc = vm.get_gc();
    let mut router = IndexMap::new();

    router.insert("get".to_string(), Value::NativeFunction(http_router_get));
    router.insert("post".to_string(), Value::NativeFunction(http_router_post));

    Ok(Value::Map(gc.allocate(pleno_value::MapState::new(router))?))
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
    log::debug!("Router: Registered POST handler");
    Ok(Value::Null)
}

// --- Server Implementation ---

pub fn http_server(
    vm: &mut dyn pleno_value::VMProvider,
    _args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    let gc = vm.get_gc();
    let mut server = IndexMap::new();

    server.insert(
        "listen".to_string(),
        Value::NativeFunction(http_server_listen),
    );

    Ok(Value::Map(gc.allocate(pleno_value::MapState::new(server))?))
}

pub fn http_server_listen(
    vm: &mut dyn pleno_value::VMProvider,
    args: &[Value],
) -> Result<Value, pleno_value::RuntimeError> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    if !vm.is_network_allowed() {
        return Err(pleno_value::RuntimeError::new(
            "SecurityError: Network access denied",
        ));
    }

    let port = match args.first() {
        Some(Value::Int(p)) => *p as u16,
        Some(Value::Float(p)) => *p as u16,
        _ => {
            return Err(pleno_value::RuntimeError::new(
                "Server.listen() expects port number",
            ));
        }
    };

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).map_err(|e| {
        pleno_value::RuntimeError::new(format!("Failed to bind to {}: {}", addr, e))
    })?;

    println!("Pleno HTTP Server listening on http://{}", addr);

    // Set non-blocking for V1 - server handles one request then returns
    // For V2, this should be a blocking event loop or async
    listener.set_nonblocking(true).ok();

    // Accept one connection for V1 demo purposes
    // In V2, this would be an event loop with Router callback support
    match listener.accept() {
        Ok((mut stream, peer_addr)) => {
            log::debug!("Connection from {}", peer_addr);

            let mut reader = BufReader::new(stream.try_clone().map_err(|e| {
                pleno_value::RuntimeError::new(format!("Stream clone failed: {}", e))
            })?);

            // Read HTTP request line
            let mut request_line = String::new();
            reader.read_line(&mut request_line).ok();

            // Parse method and path
            let parts: Vec<&str> = request_line.split_whitespace().collect();
            let (method, path) = if parts.len() >= 2 {
                (parts[0], parts[1])
            } else {
                ("GET", "/")
            };

            // Read headers until empty line
            let mut headers = IndexMap::new();
            loop {
                let mut header_line = String::new();
                if reader.read_line(&mut header_line).is_err() {
                    break;
                }
                let trimmed = header_line.trim();
                if trimmed.is_empty() {
                    break;
                }
                if let Some((key, value)) = trimmed.split_once(':') {
                    headers.insert(key.trim().to_lowercase(), value.trim().to_string());
                }
            }

            // Create request object for potential Router callback
            let gc = vm.get_gc();
            let mut req_map = IndexMap::new();
            req_map.insert(
                "method".to_string(),
                Value::String(Rc::new(method.to_string())),
            );
            req_map.insert("path".to_string(), Value::String(Rc::new(path.to_string())));

            let mut headers_map = IndexMap::new();
            for (k, v) in headers {
                headers_map.insert(k, Value::String(Rc::new(v)));
            }
            req_map.insert(
                "headers".to_string(),
                Value::Map(gc.allocate(pleno_value::MapState::new(headers_map))?),
            );

            // Default response - in V2, Router would determine this
            let response_body = format!(
                r#"{{"message":"Pleno HTTP Server","method":"{}","path":"{}"}}"#,
                method, path
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );

            stream.write_all(response.as_bytes()).map_err(|e| {
                pleno_value::RuntimeError::new(format!("Failed to write response: {}", e))
            })?;
            stream.flush().ok();

            Ok(Value::Map(
                gc.allocate(pleno_value::MapState::new(req_map))?,
            ))
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            log::trace!("Server ready, no connections yet");
            Ok(Value::Null)
        }
        Err(e) => Err(pleno_value::RuntimeError::new(format!(
            "Accept failed: {}",
            e
        ))),
    }
}
