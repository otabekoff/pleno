                Opcode::GetIndex => {
                    let collection = self.registers[base + b].clone();
                    let index = self.registers[base + c].clone();
                    let value = match (collection, index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            let idx = idx as usize;
                            array
                                .borrow()
                                .get(idx)
                                .cloned()
                                .unwrap_or(Value::Null)
                        }
                        (Value::Array(array), Value::Range(range)) => {
                            let array = array.borrow();
                            let len = array.len() as i64;
                            let start = range.start.unwrap_or(0);
                            let mut end = range.end.unwrap_or(len);
                            if range.inclusive && range.end.is_some() {
                                end += 1;
                            }

                            let start = if start < 0 {
                                (len + start).max(0)
                            } else {
                                start.min(len)
                            } as usize;
                            let end = if end < 0 {
                                (len + end).max(0)
                            } else {
                                end.min(len)
                            } as usize;

                            if start > end {
                                Value::Array(Rc::new(std::cell::RefCell::new(Vec::new())))
                            } else {
                                Value::Array(Rc::new(std::cell::RefCell::new(array[start..end].to_vec())))
                            }
                        }
                        (Value::String(s), Value::Range(range)) => {
                            let len = s.len() as i64;
                            let start = range.start.unwrap_or(0);
                            let mut end = range.end.unwrap_or(len);
                            if range.inclusive && range.end.is_some() {
                                end += 1;
                            }

                            let start = if start < 0 {
                                (len + start).max(0)
                            } else {
                                start.min(len)
                            } as usize;
                            let end = if end < 0 {
                                (len + end).max(0)
                            } else {
                                end.min(len)
                            } as usize;

                            if start > end {
                                Value::String(Rc::new("".to_string()))
                            } else {
                                // For now, we assume ASCII/UTF-8 byte slicing for simplicity
                                Value::String(Rc::new(s[start..end].to_string()))
                            }
                        }
                        (Value::Map(map), Value::String(key)) => {
                            map.borrow().get(key.as_str()).cloned().unwrap_or(Value::Null)
                        }
                        (Value::Set(set), value) => {
                            let key = ValueKey::try_from_value(&value)
                                .ok_or_else(|| RuntimeError::new("unsupported set element type"))?;
                            Value::Bool(set.borrow().contains(&key))
                        }
                        _ => return Err(RuntimeError::new("invalid index operation")),
                    };
                    self.registers[base + a] = value;
                }
                Opcode::SetIndex => {
                    let value = self.registers[base + a].clone();
                    let collection = self.registers[base + b].clone();
                    let index = self.registers[base + c].clone();
                    match (collection, index) {
                        (Value::Array(array), Value::Int(idx)) => {
                            let idx = idx as usize;
                            let mut array = array.borrow_mut();
                            if idx >= array.len() {
                                return Err(RuntimeError::new("array index out of bounds"));
                            }
                            array[idx] = value;
                        }
                        (Value::Map(map), Value::String(key)) => {
                            map.borrow_mut().insert(key.as_str().to_string(), value);
                        }
                        _ => return Err(RuntimeError::new("invalid index assignment")),
                    }
                }
