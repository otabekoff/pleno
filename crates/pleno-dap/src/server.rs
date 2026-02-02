use crate::protocol::{Event, ProtocolMessage, Request, Response};
use anyhow::Result;
use pleno_vm::VM;
use std::io::{BufRead, Write};

pub struct DapServer<R, W> {
    input: R,
    output: W,
    seq: i64,
    breakpoints: std::collections::HashMap<String, Vec<usize>>,
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
                    "supportsConditionalBreakpoints": false,
                    "supportsHitConditionalBreakpoints": false,
                    "supportsEvaluateForHovers": false,
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
                let bps = args.get("breakpoints").and_then(|v| v.as_array()).unwrap();

                let lines: Vec<usize> = bps
                    .iter()
                    .map(|b| b.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as usize)
                    .collect();
                self.breakpoints.insert(source.to_string(), lines.clone());

                let response_bps: Vec<serde_json::Value> = lines
                    .into_iter()
                    .map(|l| serde_json::json!({"verified": true, "line": l}))
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
        for (source, lines) in &self.breakpoints {
            let set: std::collections::HashSet<usize> = lines.iter().cloned().collect();
            vm.debug_context.breakpoints.insert(source.clone(), set);
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
                                let bps =
                                    args.get("breakpoints").and_then(|v| v.as_array()).unwrap();

                                let lines: Vec<usize> = bps
                                    .iter()
                                    .map(|b| {
                                        b.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as usize
                                    })
                                    .collect();
                                self.breakpoints.insert(source.to_string(), lines.clone());

                                let set: std::collections::HashSet<usize> =
                                    lines.iter().cloned().collect();
                                vm.debug_context.breakpoints.insert(source.to_string(), set);

                                let response_bps: Vec<serde_json::Value> = lines
                                    .into_iter()
                                    .map(|l| serde_json::json!({"verified": true, "line": l}))
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
                            "stackTrace" => {
                                let stack_frames: Vec<serde_json::Value> = vm
                                    .frames
                                    .iter()
                                    .enumerate()
                                    .rev()
                                    .map(|(i, frame)| {
                                        // We need function name and line info
                                        let func_name =
                                            vm.functions[frame.function_index].name.as_str();
                                        let line = if frame.ip > 0
                                            && frame.ip
                                                <= vm.functions[frame.function_index]
                                                    .chunk
                                                    .lines
                                                    .len()
                                        {
                                            vm.functions[frame.function_index].chunk.lines
                                                [frame.ip - 1]
                                        } else {
                                            0
                                        };

                                        serde_json::json!({
                                            "id": i,
                                            "name": func_name,
                                             // Add source info if available
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
                                self.send_response(&request, true, None, Some(serde_json::json!({
                                     "scopes": [
                                         {"name": "Locals", "variablesReference": 1, "expensive": false}
                                     ]
                                 })))?;
                            }
                            "variables" => {
                                self.send_response(
                                    &request,
                                    true,
                                    None,
                                    Some(serde_json::json!({
                                        "variables": []
                                    })),
                                )?;
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
}
