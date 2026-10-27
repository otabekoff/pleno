//! `std:http`: the HTTP client here, the server (`Router`, `Server`) in
//! `server`, with its static files and cookies in their own modules.

mod cookies;
mod server;
mod static_files;

pub use server::{http_router, http_server};

use indexmap::IndexMap;
use pleno_value::{MapState, RuntimeError, VMProvider, Value};
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
