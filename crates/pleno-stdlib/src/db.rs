use indexmap::IndexMap;
use pleno_value::{
    BoundNativeMethodState, DatabaseState, MapState, RuntimeError, VMProvider, Value,
};
use rusqlite::{Connection, ToSql};
use std::rc::Rc;

pub fn db_connect(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let conn_str = match args.first() {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(RuntimeError::new(
                "Database.connect expects connection string",
            ));
        }
    };

    let (driver, path) = if let Some(path) = conn_str.strip_prefix("sqlite:") {
        ("sqlite", path)
    } else {
        return Err(RuntimeError::new(
            "Unsupported database driver. Use 'sqlite:path'",
        ));
    };

    // A database file is read and written like any other file, so the
    // sandbox flags apply (`--allow-io false`, `--allow-read`, `--allow-write`).
    if !vm.is_io_allowed() {
        return Err(RuntimeError::new("SecurityError: IO access denied"));
    }
    if path != ":memory:" {
        if !vm.is_read_allowed(path) {
            return Err(RuntimeError::new("SecurityError: Read access denied"));
        }
        if !vm.is_write_allowed(path) {
            return Err(RuntimeError::new("SecurityError: Write access denied"));
        }
    }

    let conn = Connection::open(path)
        .map_err(|e| RuntimeError::new(format!("Connection failed: {}", e)))?;

    let state = DatabaseState::new(driver.to_string(), conn_str.to_string());
    *state.inner.borrow_mut() = Some(Box::new(conn));

    let gc = vm.get_gc();
    let db_val = Value::Database(gc.allocate(state)?);

    // Create a wrapper object/map with methods
    let mut db_obj = IndexMap::new();
    db_obj.insert("_handle".to_string(), db_val.clone());

    let db_map = Value::Map(gc.allocate(MapState::new(db_obj))?);

    // Bind methods
    if let Value::Map(m) = &db_map {
        let mut b = m.borrow_mut();
        let gc = vm.get_gc();
        b.entries.insert(
            "execute".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: db_map.clone(),
                function: db_execute,
            })?),
        );
        b.entries.insert(
            "query".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: db_map.clone(),
                function: db_query,
            })?),
        );
        b.entries.insert(
            "close".to_string(),
            Value::BoundNativeMethod(gc.allocate(BoundNativeMethodState {
                receiver: db_map.clone(),
                function: db_close,
            })?),
        );
    }

    Ok(db_map)
}

// SQLite implementation for std:db
pub fn db_execute(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // args[0] = this (Map), args[1] = sql, args[2] = params (Array)
    let this = &args[0];
    let sql = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("execute() expects SQL string")),
    };

    let handle = match this {
        Value::Map(m) => m.borrow().entries.get("_handle").cloned(),
        _ => return Err(RuntimeError::new("Invalid Database object")),
    };

    if let Some(Value::Database(d)) = handle {
        let state = d.borrow();
        let mut inner = state.inner.borrow_mut();
        if let Some(conn_any) = inner.as_mut()
            && let Some(conn) = conn_any.downcast_mut::<Connection>()
        {
            let sql_params = translate_params(args.get(2))?;
            let rows = conn
                .execute(sql, rusqlite::params_from_iter(sql_params))
                .map_err(|e| RuntimeError::new(format!("Execute failed: {}", e)))?;
            return Ok(Value::Int(rows as i64));
        }
    }
    Err(RuntimeError::new("Database connection is closed"))
}

#[allow(clippy::collapsible_if)]
pub fn db_query(vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    // args[0] = this (Map), args[1] = sql, args[2] = params (Array)
    let this = &args[0];
    let sql = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(RuntimeError::new("query() expects SQL string")),
    };

    let handle = match this {
        Value::Map(m) => m.borrow().entries.get("_handle").cloned(),
        _ => return Err(RuntimeError::new("Invalid Database object")),
    };

    if let Some(Value::Database(d)) = handle {
        let state = d.borrow();
        let mut inner = state.inner.borrow_mut();
        if let Some(conn_any) = inner.as_mut() {
            if let Some(conn) = conn_any.downcast_mut::<Connection>() {
                let sql_params = translate_params(args.get(2))?;
                let mut stmt = conn
                    .prepare(sql)
                    .map_err(|e| RuntimeError::new(format!("Prepare failed: {}", e)))?;

                let column_names: Vec<String> =
                    stmt.column_names().iter().map(|s| s.to_string()).collect();

                let mut rows = stmt
                    .query(rusqlite::params_from_iter(sql_params))
                    .map_err(|e| RuntimeError::new(format!("Query failed: {}", e)))?;

                let gc = vm.get_gc();
                let mut results = Vec::new();

                while let Some(row) = rows
                    .next()
                    .map_err(|e| RuntimeError::new(format!("Row fetch failed: {}", e)))?
                {
                    let mut row_map = IndexMap::new();
                    for (i, name) in column_names.iter().enumerate() {
                        let val: Value = match row
                            .get_ref(i)
                            .map_err(|e| RuntimeError::new(format!("Cell fetch failed: {}", e)))?
                        {
                            rusqlite::types::ValueRef::Null => Value::Null,
                            rusqlite::types::ValueRef::Integer(i) => Value::Int(i),
                            rusqlite::types::ValueRef::Real(f) => Value::Float(f),
                            rusqlite::types::ValueRef::Text(s) => {
                                Value::String(Rc::new(String::from_utf8_lossy(s).to_string()))
                            }
                            rusqlite::types::ValueRef::Blob(b) => {
                                let elements =
                                    b.iter().map(|&byte| Value::Int(byte as i64)).collect();
                                Value::Array(gc.allocate(pleno_value::ArrayState::new(elements))?)
                            }
                        };
                        row_map.insert(name.clone(), val);
                    }
                    results.push(Value::Map(gc.allocate(MapState::new(row_map))?));
                }

                return Ok(Value::Array(
                    gc.allocate(pleno_value::ArrayState::new(results))?,
                ));
            }
        }
    }
    Err(RuntimeError::new("Database connection is closed"))
}

pub fn db_close(_vm: &mut dyn VMProvider, args: &[Value]) -> Result<Value, RuntimeError> {
    let this = &args[0];
    let handle = match this {
        Value::Map(m) => m.borrow().entries.get("_handle").cloned(),
        _ => return Err(RuntimeError::new("Invalid Database object")),
    };

    if let Some(Value::Database(d)) = handle {
        let state = d.borrow();
        *state.inner.borrow_mut() = None;
    }
    Ok(Value::Null)
}

fn translate_params(p: Option<&Value>) -> Result<Vec<Box<dyn ToSql>>, RuntimeError> {
    let mut out: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(Value::Array(arr)) = p {
        for val in &arr.borrow().elements {
            match val {
                Value::Null => out.push(Box::new(Option::<String>::None)),
                Value::Int(i) => out.push(Box::new(*i)),
                Value::Float(f) => out.push(Box::new(*f)),
                Value::String(s) => out.push(Box::new(s.as_str().to_string())),
                Value::Bool(b) => out.push(Box::new(*b)),
                Value::Array(arr) => {
                    let mut bytes = Vec::new();
                    for v in &arr.borrow().elements {
                        if let Value::Int(i) = v {
                            if (0..=255).contains(i) {
                                bytes.push(*i as u8);
                            } else {
                                return Err(RuntimeError::new(
                                    "Array elements for BLOB must be between 0 and 255",
                                ));
                            }
                        } else {
                            return Err(RuntimeError::new(
                                "Unsupported array element type for BLOB (must be Int)",
                            ));
                        }
                    }
                    out.push(Box::new(bytes));
                }
                _ => return Err(RuntimeError::new("Unsupported parameter type in SQL")),
            }
        }
    }
    Ok(out)
}
