use indexmap::IndexMap;
use pleno_value::{MapState, RuntimeError, VMProvider, Value};
use std::net::{TcpListener, TcpStream};
use std::rc::Rc;

pub fn net_tcp_listen(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }

    let addr = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(RuntimeError::new(
                "listen() expects address string (e.g., '127.0.0.1:8080')",
            ));
        }
    };

    let _listener = TcpListener::bind(addr)
        .map_err(|e| RuntimeError::new(format!("failed to bind TCP listener: {}", e)))?;

    // For now, we'll return a mock object since we don't have a dedicated Socket type yet.
    // In a fuller implementation, this would be a GcObject wrapping the TcpListener.
    let mut obj = IndexMap::new();
    obj.insert(
        "address".to_string(),
        Value::String(Rc::new(addr.to_string())),
    );

    Ok(Value::Map(vm.get_gc().allocate(MapState::new(obj))?))
}

pub fn net_tcp_connect(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    if !vm.is_network_allowed() {
        return Err(RuntimeError::new("SecurityError: Network access denied"));
    }

    let addr = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("connect() expects address string")),
    };

    let _stream = TcpStream::connect(addr)
        .map_err(|e| RuntimeError::new(format!("failed to connect to {}: {}", addr, e)))?;

    let mut obj = IndexMap::new();
    obj.insert(
        "remoteAddress".to_string(),
        Value::String(Rc::new(addr.to_string())),
    );

    Ok(Value::Map(vm.get_gc().allocate(MapState::new(obj))?))
}
