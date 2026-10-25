//! Generic constraints: `T: Comparable` is built in and accepts numbers,
//! strings, characters and classes with `operator <`; constraints are checked
//! for inferred type arguments too; strings and characters compare with `<`.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn run(source: &str) -> String {
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
    if let Err(e) = vm.execute(chunk) {
        panic!("{source}\n{}", e.message);
    }
    vm.output.take().unwrap_or_default()
}

#[test]
fn comparable_is_built_in() {
    let output = run(
        "class Money { constructor(c) { this.c = c; } operator <(o) { return this.c < o.c; } }\n\
         class Rock {}\n\
         fn smallest<T: Comparable>(xs: T[]): T { let mut m = xs[0]; for (x in xs) { if (x < m) { m = x; } } return m; }\n\
         fn same<T: Comparable>(x: T): T { return x; }\n\
         print(smallest<Int>([3, 1, 2]), smallest([2.5, 1.5]), smallest([\"b\", \"a\"]), smallest([Money(5), Money(2)]).c);\n\
         print(same('c'), same(Money(1)).c);\n\
         try { same(Rock()); } catch (e) { print(e.message); }\n\
         try { same<Rock>(Rock()); } catch (e) { print(e.message); }",
    );
    assert_eq!(
        output,
        "1 1.5 a 2\n\
         c 1\n\
         parameter 'x' expected T: Comparable, got Rock\n\
         Generic constraint failed: Type 'Rock' does not satisfy constraint 'Comparable' for parameter 'T' in function 'same'\n"
    );
}

#[test]
fn inferred_type_arguments_are_checked() {
    let output = run("interface Named { fn name(): String; }\n\
         interface Aged { fn age(): Int; }\n\
         class Dog implements Named, Aged { fn name(): String { return \"dog\"; } fn age(): Int { return 3; } }\n\
         class Rock {}\n\
         fn greet<T: Named>(x: T): String { return x.name(); }\n\
         fn both<T: Named & Aged>(x: T): Int { return x.age(); }\n\
         print(greet(Dog()), both(Dog()));\n\
         try { greet(Rock()); } catch (e) { print(e.message); }\n\
         class Cat implements Named { fn name(): String { return \"cat\"; } }\n\
         try { both(Cat()); } catch (e) { print(e.message); }");
    assert_eq!(
        output,
        "dog 3\n\
         parameter 'x' expected T: Named, got Rock\n\
         parameter 'x' expected T: Named & Aged, got Cat\n"
    );
}

#[test]
fn strings_and_chars_compare_by_code_point() {
    let output = run(
        "print(\"abc\" < \"abd\", \"Z\" < \"a\", \"é\" > \"e\", \"\" < \"a\", \"ab\" < \"abc\", \"b\" >= \"b\");\n\
         print('a' < 'b', 'b' <= 'a');\n\
         try { print(\"a\" < 1); } catch (e) { print(e.message); }",
    );
    assert_eq!(
        output,
        "true true true true true true\ntrue false\ninvalid operands for comparison: String and Int\n"
    );
}
