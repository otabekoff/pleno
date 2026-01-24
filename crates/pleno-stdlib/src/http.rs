use pleno_value::Value;
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
