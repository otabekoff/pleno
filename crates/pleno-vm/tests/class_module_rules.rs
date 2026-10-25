//! Rules for classes and modules: imports bind only the names they list,
//! `internal` fields are visible within their module, inherited `protected`
//! fields stay inside the class hierarchy, abstract methods must be
//! implemented, and `const` fields cannot be reassigned.

use pleno_compiler::Compiler;
use pleno_value::Value;
use pleno_vm::VM;
use std::path::{Path, PathBuf};

fn run(source: &str, main_path: Option<PathBuf>) -> Result<Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.main_path = main_path;
    vm.execute(chunk).map_err(|e| e.message)
}

fn eval(source: &str) -> Result<Value, String> {
    run(source, None)
}

fn eval_string(source: &str) -> String {
    match eval(source) {
        Ok(Value::String(text)) => text.to_string(),
        other => panic!("{source}: {other:?}"),
    }
}

/// A fresh folder holding `files`, for programs that import modules.
struct ModuleDir(PathBuf);

impl ModuleDir {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "pleno-class-module-rules-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, content) in files {
            std::fs::write(dir.join(file), content).unwrap();
        }
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn run(&self, main: &str) -> Result<Value, String> {
        run(main, Some(self.path().join("main.pln")))
    }
}

impl Drop for ModuleDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const SHAPES: &str = r#"
export const PI = 3.14;
export fn square(x: Int): Int { return x * x; }
fn helper(): Int { return 7; }
export fn useHelper(): Int { return helper() + square(2); }
export class Circle {
    internal r: Int;
    constructor(r: Int) { this.r = r; }
    fn radius(): Int { return this.r; }
}
export fn readRadius(c: Circle): Int { return c.r; }
"#;

// 1. Imports bind only the names they list.

#[test]
fn named_import_does_not_leak_other_exports() {
    let dir = ModuleDir::new("leak", &[("shapes.pln", SHAPES)]);
    let err = dir
        .run("import { PI } from \"./shapes.pln\"; square(3);")
        .unwrap_err();
    assert!(err.contains("undefined variable 'square'"), "{err}");
    let err = dir
        .run("import { PI } from \"./shapes.pln\"; Circle(1);")
        .unwrap_err();
    assert!(err.contains("undefined variable 'Circle'"), "{err}");
}

#[test]
fn private_names_of_a_module_stay_private() {
    let dir = ModuleDir::new("private", &[("shapes.pln", SHAPES)]);
    let err = dir
        .run("import { useHelper } from \"./shapes.pln\"; helper();")
        .unwrap_err();
    assert!(err.contains("undefined variable 'helper'"), "{err}");
    // The module's own functions still see its private names.
    let value = dir
        .run("import { useHelper } from \"./shapes.pln\"; useHelper();")
        .unwrap();
    assert_eq!(value, Value::Int(11));
}

#[test]
fn every_import_form_still_works() {
    let dir = ModuleDir::new("forms", &[("shapes.pln", SHAPES)]);
    let value = dir
        .run(
            "import { square as sq, PI } from \"./shapes.pln\";\n\
             import Shapes from \"./shapes.pln\";\n\
             import * as S from \"./shapes.pln\";\n\
             sq(2) + Shapes.square(3) + S.square(4) + S.Circle(5).radius();",
        )
        .unwrap();
    assert_eq!(value, Value::Int(4 + 9 + 16 + 5));
}

#[test]
fn a_module_does_not_see_names_its_importer_imported() {
    let dir = ModuleDir::new(
        "nested",
        &[
            ("a.pln", "export fn fromA(): Int { return 1; }"),
            (
                "b.pln",
                "import { fromA } from \"./a.pln\";\n\
                 export fn fromB(): Int { return fromA() + 1; }",
            ),
        ],
    );
    let value = dir
        .run("import { fromB } from \"./b.pln\"; fromB();")
        .unwrap();
    assert_eq!(value, Value::Int(2));
    let err = dir
        .run("import { fromB } from \"./b.pln\"; fromA();")
        .unwrap_err();
    assert!(err.contains("undefined variable 'fromA'"), "{err}");
}

// 2. `internal` means visible within the declaring module.

#[test]
fn internal_field_is_usable_in_its_module() {
    let dir = ModuleDir::new("internal", &[("shapes.pln", SHAPES)]);
    // The constructor and methods run in the module; so does `readRadius`.
    let value = dir
        .run(
            "import { Circle, readRadius } from \"./shapes.pln\";\n\
             let c = Circle(3);\n\
             c.radius() + readRadius(c);",
        )
        .unwrap();
    assert_eq!(value, Value::Int(6));
}

#[test]
fn internal_field_is_hidden_from_other_modules() {
    let dir = ModuleDir::new("internal-hidden", &[("shapes.pln", SHAPES)]);
    let err = dir
        .run("import { Circle } from \"./shapes.pln\"; Circle(3).r;")
        .unwrap_err();
    assert!(
        err.contains("cannot read internal field 'r' of Circle outside its module"),
        "{err}"
    );
    let err = dir
        .run("import { Circle } from \"./shapes.pln\"; let c = Circle(3); c.r = 4;")
        .unwrap_err();
    assert!(
        err.contains("cannot assign to internal field 'r' of Circle outside its module"),
        "{err}"
    );
}

#[test]
fn internal_field_is_usable_in_a_single_file() {
    let value = eval("class P { internal x: Int; constructor() { this.x = 2; } } P().x;").unwrap();
    assert_eq!(value, Value::Int(2));
}

// 3. Inherited `protected` fields.

const ANIMALS: &str = "class A { protected name: String; constructor(n: String) { this.name = n; } \
                       fn fromA(): String { return this.name; } }\n\
                       class B extends A { constructor(n: String) { super(n); } \
                       fn fromB(): String { return this.name; } }\n\
                       class C extends B { fn fromC(): String { return this.name + \"!\"; } }\n";

#[test]
fn inherited_protected_field_is_hidden_outside() {
    let err = eval(&format!("{ANIMALS} B(\"x\").name;")).unwrap_err();
    assert!(
        err.contains("cannot read protected field 'name' of B outside its class"),
        "{err}"
    );
    let err = eval(&format!("{ANIMALS} let b = B(\"x\"); b.name = \"y\";")).unwrap_err();
    assert!(
        err.contains("cannot assign to protected field 'name' of B outside its class"),
        "{err}"
    );
    let err = eval(&format!("{ANIMALS} A(\"x\").name;")).unwrap_err();
    assert!(err.contains("protected field 'name' of A"), "{err}");
}

#[test]
fn protected_field_works_in_the_hierarchy() {
    let source = format!("{ANIMALS} let c = C(\"x\"); c.fromA() + c.fromB() + c.fromC();");
    assert_eq!(eval_string(&source), "xxx!");
}

#[test]
fn inherited_private_field_is_hidden_from_subclass() {
    let err = eval(
        "class A { private secret: Int; constructor() { this.secret = 1; } }\n\
         class B extends A { fn peek(): Int { return this.secret; } }\n\
         B().peek();",
    )
    .unwrap_err();
    assert!(
        err.contains("cannot read private field 'secret' of B outside its class"),
        "{err}"
    );
}

#[test]
fn field_access_errors_are_catchable_errors() {
    let source = format!(
        "{ANIMALS} let mut out = \"\"; try {{ B(\"x\").name; }} catch (e) {{ out = `${{e}}`; }} out;"
    );
    assert_eq!(
        eval_string(&source),
        "Error: cannot read protected field 'name' of B outside its class"
    );
}

// 4. Abstract methods.

const SHAPE: &str = "abstract class Shape { abstract fn area(): Int; abstract fn name(): String; \
                     fn describe(): String { return this.name() + \"=\" + `${this.area()}`; } }\n";

#[test]
fn abstract_class_cannot_be_instantiated() {
    let err = eval(&format!("{SHAPE} Shape();")).unwrap_err();
    assert!(
        err.contains("cannot instantiate abstract class 'Shape'"),
        "{err}"
    );
}

#[test]
fn subclass_must_implement_abstract_methods() {
    let err = eval(&format!(
        "{SHAPE} class Half extends Shape {{ fn area(): Int {{ return 1; }} }} Half();"
    ))
    .unwrap_err();
    assert!(
        err.contains("cannot create Half: it does not implement abstract method name"),
        "{err}"
    );
    let err = eval(&format!("{SHAPE} class Empty extends Shape {{ }} Empty();")).unwrap_err();
    assert!(
        err.contains("cannot create Empty: it does not implement abstract methods area, name"),
        "{err}"
    );
    // Named arguments construct through another path.
    let err = eval(&format!(
        "{SHAPE} class Empty extends Shape {{ constructor(x: Int) {{ }} }} Empty(x: 1);"
    ))
    .unwrap_err();
    assert!(err.contains("cannot create Empty"), "{err}");
}

#[test]
fn implemented_abstract_methods_allow_instantiation() {
    let source = format!(
        "{SHAPE} abstract class Named extends Shape {{ fn name(): String {{ return \"sq\"; }} }}\n\
         class Square extends Named {{ fn area(): Int {{ return 4; }} }}\n\
         Square().describe();"
    );
    assert_eq!(eval_string(&source), "sq=4");
}

#[test]
fn abstract_method_in_concrete_class_is_a_compile_error() {
    let err = eval("class Shape { abstract fn area(): Int; }").unwrap_err();
    assert!(
        err.contains("abstract method 'area' in class 'Shape', which is not abstract"),
        "{err}"
    );
}

#[test]
fn missing_abstract_method_error_is_catchable() {
    let source = format!(
        "{SHAPE} class Empty extends Shape {{ }}\n\
         let mut out = \"\"; try {{ Empty(); }} catch (e) {{ out = e.name + \": \" + e.message; }} out;"
    );
    assert_eq!(
        eval_string(&source),
        "Error: cannot create Empty: it does not implement abstract methods area, name"
    );
}

// 5. `const` fields.

#[test]
fn static_const_cannot_be_reassigned() {
    let err = eval(
        "class MathUtils { public static const PI: Float = 3.14159; }\n\
         MathUtils.PI = 3.0;",
    )
    .unwrap_err();
    assert!(
        err.contains("cannot assign to constant MathUtils.PI"),
        "{err}"
    );
    // Not from a static method of the class either.
    let err = eval(
        "class MathUtils { public static const PI: Float = 3.14159; \
         static fn reset() { MathUtils.PI = 3.0; } }\n\
         MathUtils.reset();",
    )
    .unwrap_err();
    assert!(
        err.contains("cannot assign to constant MathUtils.PI"),
        "{err}"
    );
}

#[test]
fn static_const_and_static_fields_still_work() {
    let value = eval(
        "class Counter { public static const STEP: Int = 2; static count: Int = 0;\n\
         static fn bump() { Counter.count = Counter.count + Counter.STEP; } }\n\
         Counter.bump(); Counter.bump(); Counter.count;",
    )
    .unwrap();
    assert_eq!(value, Value::Int(4));
}

#[test]
fn const_instance_field_is_set_only_by_constructors() {
    let class = "class Point { const x: Int; constructor(x: Int) { this.x = x; } \
                 fn reset() { this.x = 0; } }\n\
                 class Point3 extends Point { constructor(x: Int) { super(x); this.x = x + 1; } }\n";
    assert_eq!(
        eval(&format!("{class} Point(1).x + Point3(1).x;")).unwrap(),
        Value::Int(3)
    );
    let err = eval(&format!("{class} let p = Point(1); p.x = 2;")).unwrap_err();
    assert!(
        err.contains("cannot assign to constant field 'x' of Point outside its constructor"),
        "{err}"
    );
    let err = eval(&format!("{class} Point(1).reset();")).unwrap_err();
    assert!(err.contains("cannot assign to constant field 'x'"), "{err}");
}
