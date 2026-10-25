//! Sandbox path rules compare normalized paths: `..` cannot leave an allowed
//! folder, a sibling with a longer name is not inside it, and checking
//! whether a file exists counts as reading it.

use pleno_compiler::Compiler;
use pleno_value::SandboxConfig;
use pleno_vm::VM;
use std::path::PathBuf;

fn run_with_read_rule(source: &str, allowed: &str) -> String {
    let tokens = pleno_lexer::lex(source).unwrap();
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .unwrap();
    let chunk = Compiler::new(source.to_string())
        .compile(&mut program)
        .unwrap();
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    vm.apply_config(SandboxConfig {
        allow_io: true,
        allow_network: false,
        max_memory: 0,
        max_instructions: 0,
        allowed_read_paths: vec![allowed.to_string()],
        allowed_write_paths: vec![allowed.to_string()],
    });
    vm.execute(chunk).unwrap();
    vm.output.take().unwrap_or_default()
}

fn fixture() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pleno-sandbox-paths-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::create_dir_all(dir.join("data2")).unwrap();
    std::fs::write(dir.join("secret.txt"), "secret").unwrap();
    std::fs::write(dir.join("data/ok.txt"), "ok").unwrap();
    std::fs::write(dir.join("data2/x.txt"), "x").unwrap();
    dir
}

#[test]
fn path_rules_compare_normalized_paths() {
    let dir = fixture();
    let root = dir
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let source = format!(
        "import {{ File }} from \"std:io\";\n\
         import {{ Path }} from \"std:path\";\n\
         fn read(path) {{ try {{ return File.read(path); }} catch (e) {{ return e.name; }} }}\n\
         fn exists(path) {{ try {{ return Path.exists(path); }} catch (e) {{ return e.name; }} }}\n\
         print(read(\"{root}/data/ok.txt\"));\n\
         print(read(\"{root}/data/./ok.txt\"));\n\
         print(read(\"{root}/data/../secret.txt\"));\n\
         print(read(\"{root}/data2/x.txt\"));\n\
         print(exists(\"{root}/secret.txt\"));\n\
         print(exists(\"{root}/data/ok.txt\"));\n\
         print(exists(\"{root}/data/new.txt\"));"
    );
    let output = run_with_read_rule(&source, &format!("{root}/data"));
    assert_eq!(
        output, "ok\nok\nSecurityError\nSecurityError\nSecurityError\ntrue\nfalse\n",
        "{source}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
