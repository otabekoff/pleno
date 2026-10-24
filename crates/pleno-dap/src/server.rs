use crate::protocol::{Event, ProtocolMessage, Request, Response};
use anyhow::Result;
use pleno_vm::VM;
use std::io::{BufRead, Write};

pub struct DapServer<R, W> {
    input: R,
    output: W,
    seq: i64,
    breakpoints: std::collections::HashMap<String, Vec<pleno_vm::Breakpoint>>,
    /// A launched program waiting for `configurationDone` before it runs.
    pending_launch: Option<VM>,
    configuration_done: bool,
}

/// Breakpoint paths from the client and the VM's script path must compare equal.
fn normalize_path(path: &str) -> String {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

impl<R: BufRead, W: Write> DapServer<R, W> {
    pub fn new(input: R, output: W) -> Self {
        Self {
            input,
            output,
            seq: 1,
            breakpoints: std::collections::HashMap::new(),
            pending_launch: None,
            configuration_done: false,
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
                    "supportsTerminateRequest": true,
                    "exceptionBreakpointFilters": [],
                });
                self.send_response(&request, true, None, Some(body))?;
                self.send_event("initialized".to_string(), None)?;
            }
            "launch" => {
                self.handle_launch(request)?;
            }
            "configurationDone" => {
                self.send_response(&request, true, None, None)?;
                self.configuration_done = true;
                self.start_pending_launch()?;
            }
            "setExceptionBreakpoints" => {
                self.send_response(
                    &request,
                    true,
                    None,
                    Some(serde_json::json!({ "breakpoints": [] })),
                )?;
            }
            "threads" => {
                self.send_response(
                    &request,
                    true,
                    None,
                    Some(serde_json::json!({ "threads": [{"id": 1, "name": "main"}] })),
                )?;
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

                self.breakpoints
                    .insert(normalize_path(source), vm_bps.clone());

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
            "disconnect" | "terminate" => {
                self.send_response(&request, true, None, None)?;
                return Ok(());
            }
            _ => {
                let message = format!("Unsupported request: {}", request.command);
                self.send_response(&request, false, Some(message), None)?;
            }
        }
        Ok(())
    }

    /// Runs a launched program once the client has finished configuring
    /// breakpoints (`configurationDone`).
    fn start_pending_launch(&mut self) -> Result<()> {
        if !self.configuration_done {
            return Ok(());
        }
        if let Some(mut vm) = self.pending_launch.take() {
            for (source, bps) in &self.breakpoints {
                vm.debug_context
                    .set_breakpoints(source.clone(), bps.clone());
            }
            self.debug_execution_loop(&mut vm)?;
            self.send_event("terminated".to_string(), None)?;
        }
        Ok(())
    }

    /// Sends program output captured by the VM to the client.
    fn flush_output(&mut self, vm: &mut VM) -> Result<()> {
        let text = vm.output.as_mut().map(std::mem::take).unwrap_or_default();
        if !text.is_empty() {
            self.send_event(
                "output".to_string(),
                Some(serde_json::json!({ "category": "stdout", "output": text })),
            )?;
        }
        Ok(())
    }

    /// Reports a runtime error to the client and ends the session.
    fn report_error(&mut self, vm: &mut VM, error: pleno_value::RuntimeError) -> Result<()> {
        self.flush_output(vm)?;
        self.send_event(
            "output".to_string(),
            Some(serde_json::json!({
                "category": "stderr",
                "output": format!("Error: {}\n", error.message)
            })),
        )?;
        self.send_event(
            "exited".to_string(),
            Some(serde_json::json!({"exitCode": 1})),
        )
    }

    fn handle_launch(&mut self, request: Request) -> Result<()> {
        let args = request.arguments.clone().unwrap_or(serde_json::Value::Null);
        let program = args
            .get("program")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing program path"))?;

        let compiled = std::fs::read_to_string(program)
            .map_err(|e| format!("Cannot read {program}: {e}"))
            .and_then(|source| {
                let tokens = pleno_lexer::lex(&source)
                    .map_err(|e| format!("Syntax error: {}", e.message))?;
                let mut program_ast = pleno_parser::Parser::new(tokens, &source)
                    .parse_program()
                    .map_err(|e| format!("Syntax error: {}", e.message))?;
                pleno_compiler::Compiler::new(source)
                    .compile(&mut program_ast)
                    .map_err(|e| format!("Compile error: {}", e.diagnostic.message))
            });
        let chunk = match compiled {
            Ok(chunk) => chunk,
            Err(message) => {
                self.send_response(&request, false, Some(message), None)?;
                return Ok(());
            }
        };

        let mut vm = VM::new();
        vm.main_path = std::fs::canonicalize(program).ok();
        pleno_stdlib::register(&mut vm);
        // Capture program output so it is sent as DAP events, not mixed into
        // the protocol stream on stdout.
        vm.output = Some(String::new());
        vm.load_script_with_path(chunk, Some(normalize_path(program)));

        self.send_response(&request, true, None, None)?;
        self.send_event(
            "process".to_string(),
            Some(serde_json::json!({"name": program})),
        )?;

        self.pending_launch = Some(vm);
        self.start_pending_launch()
    }

    fn debug_execution_loop(&mut self, vm: &mut VM) -> Result<()> {
        let mut status = match vm.run_debug() {
            Ok(status) => status,
            Err(error) => return self.report_error(vm, error),
        };

        loop {
            self.flush_output(vm)?;
            match status {
                pleno_vm::ExecutionStatus::Finished(_) => {
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
                                self.send_response(&request, true, None, None)?;
                                status = match vm.run_debug() {
                                    Ok(status) => status,
                                    Err(error) => return self.report_error(vm, error),
                                };
                                break;
                            }
                            "next" => {
                                self.send_response(&request, true, None, None)?;
                                status = match vm.step_over() {
                                    Ok(status) => status,
                                    Err(error) => return self.report_error(vm, error),
                                };
                                break;
                            }
                            "stepIn" => {
                                self.send_response(&request, true, None, None)?;
                                status = match vm.step_into() {
                                    Ok(status) => status,
                                    Err(error) => return self.report_error(vm, error),
                                };
                                break;
                            }
                            "stepOut" => {
                                self.send_response(&request, true, None, None)?;
                                status = match vm.step_out() {
                                    Ok(status) => status,
                                    Err(error) => return self.report_error(vm, error),
                                };
                                break;
                            }
                            "disconnect" | "terminate" => {
                                self.send_response(&request, true, None, None)?;
                                return Ok(());
                            }
                            "configurationDone" | "pause" => {
                                self.send_response(&request, true, None, None)?;
                            }
                            "setExceptionBreakpoints" => {
                                self.send_response(
                                    &request,
                                    true,
                                    None,
                                    Some(serde_json::json!({ "breakpoints": [] })),
                                )?;
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

                                let source = normalize_path(source);
                                self.breakpoints.insert(source.clone(), vm_bps.clone());
                                vm.debug_context.set_breakpoints(source, vm_bps.clone());

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
                                let top = vm.frames.len().saturating_sub(1);
                                let stack_frames: Vec<serde_json::Value> = vm
                                    .frames
                                    .iter()
                                    .enumerate()
                                    .rev()
                                    .map(|(i, frame)| {
                                        let function = &vm.functions[frame.function_index];
                                        let func_name = function.name.as_str();
                                        // The paused frame is about to run `ip`; callers have
                                        // already run their call instruction at `ip - 1`.
                                        let ip = if i == top {
                                            frame.ip
                                        } else {
                                            frame.ip.saturating_sub(1)
                                        };
                                        let line = function.chunk.lines.get(ip).copied().unwrap_or(0);

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
                                let message = format!("Unsupported request: {}", request.command);
                                self.send_response(&request, false, Some(message), None)?;
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
