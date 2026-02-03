use crate::protocol::{Event, ProtocolMessage, Request, Response};
use anyhow::Result;
use pleno_vm::VM;
use std::io::{BufRead, Write};

pub struct DapServer<R, W> {
    input: R,
    output: W,
    seq: i64,
    breakpoints: std::collections::HashMap<String, Vec<pleno_vm::Breakpoint>>,
}

impl<R: BufRead, W: Write> DapServer<R, W> {
    pub fn new(input: R, output: W) -> Self {
        Self {
            input,
            output,
            seq: 1,
            breakpoints: std::collections::HashMap::new(),
        }
    }

    pub fn send_response(
        &mut self,
        request: &Request,
        success: bool,
        message: Option<String>,
        body: Option<serde_json::Value>,
    ) -> Result<()> {
        let response = Response {
            seq: self.seq,
            request_seq: request.seq,
            success,
            command: request.command.clone(),
            message,
            body,
        };
        self.seq += 1;
        self.send_message(ProtocolMessage::Response(response))
    }

    pub fn send_event(&mut self, event: String, body: Option<serde_json::Value>) -> Result<()> {
        let event = Event {
            seq: self.seq,
            event,
            body,
        };
        self.seq += 1;
        self.send_message(ProtocolMessage::Event(event))
    }

    fn send_message(&mut self, message: ProtocolMessage) -> Result<()> {
        let json = serde_json::to_string(&message)?;
        let content_length = json.len();
        write!(
            self.output,
            "Content-Length: {}\r\n\r\n{}",
            content_length, json
        )?;
        self.output.flush()?;
        Ok(())
    }

    pub fn read_request(&mut self) -> Result<Option<Request>> {
        let mut content_length = 0;
        let mut header = String::new();

        loop {
            header.clear();
            if self.input.read_line(&mut header)? == 0 {
                return Ok(None); // EOF
            }
            let trimmed = header.trim();
            if trimmed.is_empty() {
                break; // End of headers
            }
            if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                content_length = rest.trim().parse::<usize>().unwrap_or(0);
            }
        }

        if content_length == 0 {
            return Ok(None);
        }

        let mut body_buffer = vec![0; content_length];
        self.input.read_exact(&mut body_buffer)?;

        let body_str = String::from_utf8(body_buffer)?;
        let message: ProtocolMessage = serde_json::from_str(&body_str)?;

        match message {
            ProtocolMessage::Request(req) => Ok(Some(req)),
            _ => Ok(None), // Filter out responses/events if any (loopback?)
        }
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            let request = match self.read_request()? {
                Some(req) => req,
                None => return Ok(()),
            };
            self.handle_request(request)?;
        }
    }

    fn handle_request(&mut self, request: Request) -> Result<()> {
        match request.command.as_str() {
            "initialize" => {
                let body = serde_json::json!({
                    "supportsConfigurationDoneRequest": true,
                    "supportsFunctionBreakpoints": false,
                    "supportsConditionalBreakpoints": true,
                    "supportsHitConditionalBreakpoints": false,
                    "supportsEvaluateForHovers": true,
                    "supportsDataBreakpoints": true,
                });
                self.send_response(&request, true, None, Some(body))?;
                self.send_event("initialized".to_string(), None)?;
            }
            "launch" => {
                self.handle_launch(request)?;
            }
            "setBreakpoints" => {
                let args = request.arguments.as_ref().unwrap();
                let source = args
                    .get("source")
                    .and_then(|s| s.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let bps_json = args.get("breakpoints").and_then(|v| v.as_array()).unwrap();

                let mut vm_bps = Vec::new();
                for bp_j in bps_json {
                    let line = bp_j.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as usize;
                    let condition = bp_j
                        .get("condition")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string());
                    let hit_condition = bp_j
                        .get("hitCondition")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string());
                    let log_message = bp_j
                        .get("logMessage")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string());

                    vm_bps.push(pleno_vm::Breakpoint {
                        line,
                        condition,
                        hit_condition,
                        log_message,
                    });
                }

                self.breakpoints.insert(source.to_string(), vm_bps.clone());

                let response_bps: Vec<serde_json::Value> = vm_bps
                    .into_iter()
                    .map(|b| serde_json::json!({"verified": true, "line": b.line}))
                    .collect();

                self.send_response(
                    &request,
                    true,
                    None,
                    Some(serde_json::json!({
                        "breakpoints": response_bps
                    })),
                )?;
            }
            "dataBreakpointInfo" => {
                let args = request.arguments.as_ref().unwrap();
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");

                self.send_response(
                    &request,
                    true,
                    None,
                    Some(serde_json::json!({
                        "dataId": name,
                        "description": format!("Value change of '{}'", name),
                        "accessTypes": ["write"],
                        "canPersist": true
                    })),
                )?;
            }
            "setDataBreakpoints" => {
                let args = request.arguments.as_ref().unwrap();
                let bps_json = args.get("breakpoints").and_then(|v| v.as_array()).unwrap();

                // Clear or maintain a separate data_breakpoints list in DapServer?
                // For now, just a placeholder if needed, but VM will hold the active ones.
                // We don't have a place to store them in DapServer yet besides the VM itself which isn't created.
                // But we can store them in a field similar to self.breakpoints.

                let mut response_bps = Vec::new();
                for bp_j in bps_json {
                    let _data_id = bp_j.get("dataId").and_then(|v| v.as_str()).unwrap_or("");
                    response_bps.push(serde_json::json!({
                        "verified": true
                    }));
                }

                self.send_response(
                    &request,
                    true,
                    None,
                    Some(serde_json::json!({
                        "breakpoints": response_bps
                    })),
                )?;
            }
            "disconnect" => {
                self.send_response(&request, true, None, None)?;
                return Ok(());
            }
            _ => {
                self.send_response(&request, false, Some("Not implemented".to_string()), None)?;
            }
        }
        Ok(())
    }

    fn handle_launch(&mut self, request: Request) -> Result<()> {
        let args = request.arguments.clone().unwrap_or(serde_json::Value::Null);
        let program = args
            .get("program")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing program path"))?;

        let source = std::fs::read_to_string(program)?;

        // Lex
        let tokens =
            pleno_lexer::lex(&source).map_err(|e| anyhow::anyhow!("Lexing failed: {:?}", e))?;

        // Parse
        let mut program_ast = pleno_parser::Parser::new(tokens, &source)
            .parse_program()
            .map_err(|e| anyhow::anyhow!("Parsing failed: {:?}", e))?;

        // Compile
        let mut compiler = pleno_compiler::Compiler::new(source);
        let chunk = compiler
            .compile(&mut program_ast)
            .map_err(|e| anyhow::anyhow!("Compilation failed: {:?}", e))?;

        let mut vm = VM::new();
        for (source, bps) in &self.breakpoints {
            vm.debug_context
                .set_breakpoints(source.clone(), bps.clone());
        }
        vm.load_script(chunk);

        self.send_response(&request, true, None, None)?;
        self.send_event(
            "process".to_string(),
            Some(serde_json::json!({"name": program})),
        )?;

        self.debug_execution_loop(&mut vm)?;

        self.send_event("terminated".to_string(), None)?;
        Ok(())
    }

    fn debug_execution_loop(&mut self, vm: &mut VM) -> Result<()> {
        let mut status = vm
            .run_debug()
            .map_err(|e| anyhow::anyhow!("VM Error: {:?}", e))?;

        loop {
            match status {
                pleno_vm::ExecutionStatus::Finished(val) => {
                    let message = format!("Program exited with: {:?}", val);
                    self.send_event(
                        "output".to_string(),
                        Some(serde_json::json!({
                            "category": "stdout",
                            "output": message
                        })),
                    )?;
                    self.send_event(
                        "exited".to_string(),
                        Some(serde_json::json!({"exitCode": 0})),
                    )?;
                    return Ok(());
                }
                pleno_vm::ExecutionStatus::Paused => {
                    vm.debug_context.variable_tracker.clear();
                    self.send_event(
                        "stopped".to_string(),
                        Some(serde_json::json!({
                            "reason": "breakpoint",
                            "threadId": 1,
                            "allThreadsStopped": true
                        })),
                    )?;

                    loop {
                        let request = match self.read_request()? {
                            Some(r) => r,
                            None => return Ok(()),
                        };

                        match request.command.as_str() {
                            "continue" => {
                                status = vm
                                    .run_debug()
                                    .map_err(|e| anyhow::anyhow!("VM Error: {:?}", e))?;
                                self.send_response(&request, true, None, None)?;
                                break;
                            }
                            "next" => {
                                status = vm
                                    .step_over()
                                    .map_err(|e| anyhow::anyhow!("VM Error: {:?}", e))?;
                                self.send_response(&request, true, None, None)?;
                                break;
                            }
                            "stepIn" => {
                                status = vm
                                    .step_into()
                                    .map_err(|e| anyhow::anyhow!("VM Error: {:?}", e))?;
                                self.send_response(&request, true, None, None)?;
                                break;
                            }
                            "stepOut" => {
                                status = vm
                                    .step_out()
                                    .map_err(|e| anyhow::anyhow!("VM Error: {:?}", e))?;
                                self.send_response(&request, true, None, None)?;
                                break;
                            }
                            "disconnect" | "terminate" => {
                                self.send_response(&request, true, None, None)?;
                                return Ok(());
                            }
                            "threads" => {
                                self.send_response(
                                    &request,
                                    true,
                                    None,
                                    Some(serde_json::json!({
                                        "threads": [{"id": 1, "name": "main"}]
                                    })),
                                )?;
                            }
                            "setBreakpoints" => {
                                let args = request.arguments.as_ref().unwrap();
                                let source = args
                                    .get("source")
                                    .and_then(|s| s.get("path"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let bps_json =
                                    args.get("breakpoints").and_then(|v| v.as_array()).unwrap();

                                let mut vm_bps = Vec::new();
                                for bp_j in bps_json {
                                    let line =
                                        bp_j.get("line").and_then(|l| l.as_u64()).unwrap_or(0)
                                            as usize;
                                    let condition = bp_j
                                        .get("condition")
                                        .and_then(|c| c.as_str())
                                        .map(|s| s.to_string());
                                    let hit_condition = bp_j
                                        .get("hitCondition")
                                        .and_then(|c| c.as_str())
                                        .map(|s| s.to_string());
                                    let log_message = bp_j
                                        .get("logMessage")
                                        .and_then(|c| c.as_str())
                                        .map(|s| s.to_string());

                                    vm_bps.push(pleno_vm::Breakpoint {
                                        line,
                                        condition,
                                        hit_condition,
                                        log_message,
                                    });
                                }

                                self.breakpoints.insert(source.to_string(), vm_bps.clone());
                                vm.debug_context
                                    .set_breakpoints(source.to_string(), vm_bps.clone());

                                let response_bps: Vec<serde_json::Value> = vm_bps
                                    .into_iter()
                                    .map(|b| serde_json::json!({"verified": true, "line": b.line}))
                                    .collect();

                                self.send_response(
                                    &request,
                                    true,
                                    None,
                                    Some(serde_json::json!({
                                        "breakpoints": response_bps
                                    })),
                                )?;
                            }
                            "dataBreakpointInfo" => {
                                let args = request.arguments.as_ref().unwrap();
                                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");

                                self.send_response(
                                    &request,
                                    true,
                                    None,
                                    Some(serde_json::json!({
                                        "dataId": name,
                                        "description": format!("Value change of '{}'", name),
                                        "accessTypes": ["write"],
                                        "canPersist": true
                                    })),
                                )?;
                            }
                            "setDataBreakpoints" => {
                                let args = request.arguments.as_ref().unwrap();
                                let bps_json =
                                    args.get("breakpoints").and_then(|v| v.as_array()).unwrap();

                                vm.debug_context.data_breakpoints.clear();
                                let mut response_bps = Vec::new();

                                for bp_j in bps_json {
                                    let data_id =
                                        bp_j.get("dataId").and_then(|v| v.as_str()).unwrap_or("");
                                    vm.debug_context
                                        .data_breakpoints
                                        .insert(data_id.to_string(), pleno_value::Value::Null);
                                    response_bps.push(serde_json::json!({
                                        "verified": true
                                    }));
                                }

                                self.send_response(
                                    &request,
                                    true,
                                    None,
                                    Some(serde_json::json!({
                                        "breakpoints": response_bps
                                    })),
                                )?;
                            }
                            "stackTrace" => {
                                let stack_frames: Vec<serde_json::Value> = vm
                                    .frames
                                    .iter()
                                    .enumerate()
                                    .rev()
                                    .map(|(i, frame)| {
                                        let function = &vm.functions[frame.function_index];
                                        let func_name = function.name.as_str();
                                        let line = if frame.ip > 0
                                            && frame.ip <= function.chunk.lines.len()
                                        {
                                            function.chunk.lines[frame.ip - 1]
                                        } else {
                                            0
                                        };

                                        let mut source_json = serde_json::json!({});
                                        if let Some(path) = &function.module_path {
                                            source_json = serde_json::json!({
                                                "name": std::path::Path::new(path.as_str()).file_name().and_then(|s| s.to_str()).unwrap_or("unknown"),
                                                "path": path,
                                            });
                                        }

                                        serde_json::json!({
                                            "id": i,
                                            "name": func_name,
                                            "source": source_json,
                                            "line": line,
                                            "column": 0
                                        })
                                    })
                                    .collect();

                                self.send_response(
                                    &request,
                                    true,
                                    None,
                                    Some(serde_json::json!({
                                        "stackFrames": stack_frames,
                                        "totalFrames": stack_frames.len()
                                    })),
                                )?;
                            }
                            "scopes" => {
                                let frame_id = request
                                    .arguments
                                    .as_ref()
                                    .and_then(|a| a.get("frameId"))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                // Specially encoded IDs for top-level scopes
                                let locals_ref = 1_000_000 + (frame_id + 1) * 10 + 1;
                                let globals_ref = 1_000_000 + (frame_id + 1) * 10 + 2;

                                self.send_response(
                                    &request,
                                    true,
                                    None,
                                    Some(serde_json::json!({
                                        "scopes": [
                                            {"name": "Locals", "variablesReference": locals_ref, "expensive": false},
                                            {"name": "Globals", "variablesReference": globals_ref, "expensive": false}
                                        ]
                                    })),
                                )?;
                            }
                            "variables" => {
                                let var_ref = request
                                    .arguments
                                    .as_ref()
                                    .and_then(|a| a.get("variablesReference"))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);

                                let mut variables = Vec::new();

                                if var_ref >= 1_000_000 {
                                    let frame_idx = ((var_ref - 1_000_000) / 10) - 1;
                                    let scope_type = var_ref % 10;

                                    if let Some(frame) = vm.frames.get(frame_idx as usize) {
                                        if scope_type == 1 {
                                            // Locals
                                            let func = &vm.functions[frame.function_index];
                                            let ip = frame.ip;
                                            let mut found_locals = Vec::new();

                                            for local in &func.chunk.locals {
                                                if ip >= local.start_ip && ip <= local.end_ip {
                                                    let val = vm.registers
                                                        [frame.base + local.register as usize]
                                                        .to_value();
                                                    found_locals.push((local.name.clone(), val));
                                                }
                                            }

                                            let mut found_params = Vec::new();
                                            for (i, param) in func.params.iter().enumerate() {
                                                let val = vm.registers[frame.base + i].to_value();
                                                found_params.push((param.name.clone(), val));
                                            }

                                            for (name, val) in found_locals {
                                                variables.push(
                                                    self.create_variable_json(vm, &name, val),
                                                );
                                            }
                                            for (name, val) in found_params {
                                                variables.push(
                                                    self.create_variable_json(vm, &name, val),
                                                );
                                            }
                                        } else if scope_type == 2 {
                                            // Globals
                                            let globals: Vec<_> = vm
                                                .globals
                                                .iter()
                                                .map(|(k, v)| (k.clone(), v.clone()))
                                                .collect();
                                            for (name, val) in globals {
                                                variables.push(
                                                    self.create_variable_json(vm, &name, val),
                                                );
                                            }
                                        }
                                    }
                                } else {
                                    // Structured value expansion
                                    let children = vm.get_variable_children(var_ref);
                                    for (name, val) in children {
                                        variables.push(self.create_variable_json(vm, &name, val));
                                    }
                                }

                                self.send_response(
                                    &request,
                                    true,
                                    None,
                                    Some(serde_json::json!({
                                        "variables": variables
                                    })),
                                )?;
                            }
                            "evaluate" => {
                                let args = request.arguments.as_ref().unwrap();
                                let expression = args
                                    .get("expression")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                match vm.evaluate_expression(expression) {
                                    Ok(val) => {
                                        self.send_response(
                                            &request,
                                            true,
                                            None,
                                            Some(serde_json::json!({
                                                "result": format!("{:?}", val),
                                                "variablesReference": self.get_value_ref(vm, val)
                                            })),
                                        )?;
                                    }
                                    Err(e) => {
                                        self.send_response(&request, false, Some(e.message), None)?;
                                    }
                                }
                            }
                            _ => {
                                self.send_response(
                                    &request,
                                    false,
                                    Some("Not implemented".to_string()),
                                    None,
                                )?;
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_value_ref(&self, vm: &mut VM, val: pleno_value::Value) -> u64 {
        match val {
            pleno_value::Value::Array(_)
            | pleno_value::Value::Map(_)
            | pleno_value::Value::Object(_)
            | pleno_value::Value::Tuple(_) => vm.debug_context.variable_tracker.track(val),
            _ => 0,
        }
    }

    fn create_variable_json(
        &self,
        vm: &mut VM,
        name: &str,
        val: pleno_value::Value,
    ) -> serde_json::Value {
        let var_ref = self.get_value_ref(vm, val.clone());
        serde_json::json!({
            "name": name,
            "value": format!("{:?}", val),
            "variablesReference": var_ref
        })
    }
}
