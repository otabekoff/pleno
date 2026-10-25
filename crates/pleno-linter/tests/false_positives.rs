use pleno_linter::lint;

#[test]
fn test_gc_import_no_undef() {
    let input = r#"
        import GC from "std:gc";
        GC.report();
        GC.collect();
        let stats = GC.stats();
    "#;
    let messages = lint(input);

    // Should NOT have "undefined variable 'GC'"
    assert!(
        !messages
            .iter()
            .any(|msg| { msg.rule_name == "no-undef" && msg.message.contains("'GC'") }),
        "GC should be defined via import"
    );
}

#[test]
fn test_array_push_usage() {
    let input = r#"
        fn test() {
            let arr = [];
            arr.push(1);
        }
    "#;
    let messages = lint(input);

    // Should NOT have "variable 'arr' is declared but never used"
    assert!(
        !messages
            .iter()
            .any(|msg| { msg.rule_name == "no-unused-vars" && msg.message.contains("'arr'") }),
        "arr should be considered used via .push()"
    );
}

#[test]
fn test_cfor_variable_scope() {
    let input = r#"
        fn test() {
            for (let mut i = 0; i < 10; i++) {
                print(i);
            }
        }
    "#;
    let messages = lint(input);

    // Should NOT have "undefined variable 'i'" or "unused variable 'i'"
    assert!(
        !messages.iter().any(|msg| {
            (msg.rule_name == "no-undef" || msg.rule_name == "no-unused-vars")
                && msg.message.contains("'i'")
        }),
        "i should be correctly scoped and used in CFor loop"
    );
}

#[test]
fn test_import_kinds_resolution() {
    let input = r#"
        import { add, subtract as sub } from "math";
        import * as math_all from "math";
        
        add(1, 2);
        sub(3, 4);
        math_all.pi;
    "#;
    let messages = lint(input);

    assert!(
        !messages.iter().any(|msg| msg.rule_name == "no-undef"),
        "All imports should be correctly resolved"
    );
}

#[test]
fn test_complex_literals_usage() {
    let input = r#"
        let x = 1;
        let y = 2;
        let arr = [x, y];
        let map = {"a": x, "b": y};
        let tuple = (x, y);
        let named = (a: x, b: y);
        let set = {x, y};
    "#;
    let messages = lint(input);

    // `x` and `y` are only read inside literals; the containers themselves are unused.
    assert!(
        !messages.iter().any(|msg| msg.rule_name == "no-unused-vars"
            && (msg.message.contains("'x'") || msg.message.contains("'y'"))),
        "Variables used in literals should be marked as used"
    );
}

fn rules(input: &str, rule: &str) -> Vec<String> {
    lint(input)
        .into_iter()
        .filter(|msg| msg.rule_name == rule)
        .map(|msg| format!("{}:{} {}", msg.line, msg.column, msg.message))
        .collect()
}

#[test]
fn test_lambdas_and_generics_are_not_operators() {
    let input = r#"
async fn delay(ms: Int): Promise<Int> { return ms; }
let double = (x) => x * 2;
let pairs: Map<String, Int[]> = Map();
let typed = fn(): Promise<Int> => delay(1);
print(double(2), pairs, typed, `${1+2}`);
"#;
    assert_eq!(rules(input, "operator-spacing"), Vec::<String>::new());
}

#[test]
fn test_operator_spacing_is_reported_at_the_operator() {
    let input =
        "let a = 1;\nlet b=a+1;\nlet c = a != b && b>a;\nlet mut d = 0;\nd+=c;\nprint(d);\n";
    assert_eq!(
        rules(input, "operator-spacing"),
        vec![
            "2:6 Missing spacing around operator '='",
            "2:8 Missing spacing around operator '+'",
            "3:20 Missing spacing around operator '>'",
            "5:2 Missing spacing around operator '+='",
        ]
    );
}

#[test]
fn test_names_used_only_in_types_are_used() {
    let input = r#"
type Money = Float;
interface Named { fn name(label: String): String; }
class Wallet implements Named {
    amount: Money;
    constructor(a) { this.amount = a; }
    fn name(label: String): String { return label; }
}
let m: Money? = 4;
print(Wallet(m).name("w"));
"#;
    assert_eq!(rules(input, "no-unused-vars"), Vec::<String>::new());
}

#[test]
fn test_functions_can_be_used_before_their_declaration() {
    let input = r#"
print(later(2));
fn later(x) { return helper(x); }
fn helper(x) { return x; }
"#;
    assert_eq!(rules(input, "no-unused-vars"), Vec::<String>::new());
    assert_eq!(rules(input, "no-undef"), Vec::<String>::new());
}

#[test]
fn test_standard_library_globals_are_defined() {
    let input = r#"
let s = Set([1, 2]);
let r: Result<Int, String> = Ok(1);
let o = Some(2);
let e = Error("boom");
print(s, r, o, e, None, Err("x"));
"#;
    assert_eq!(rules(input, "no-undef"), Vec::<String>::new());
    assert_eq!(
        rules("print(missing);\n", "no-undef"),
        vec!["1:7 Usage of undefined variable 'missing'"]
    );
}

#[test]
fn test_unused_parameters_before_a_used_one_are_not_reported() {
    let input = r#"
let handler = fn(req, res) { return res; };
fn unusedLast(a, b) { return a; }
print(handler(1, 2), unusedLast(1, 2));
"#;
    assert_eq!(
        rules(input, "no-unused-vars"),
        vec!["3:18 Variable 'b' is declared but never used"]
    );
}

#[test]
fn test_exports_enum_variants_and_modules_are_used() {
    let input = r#"
module Shapes;
export fn area(r) { return r * r; }
fn perimeter(r) { return r * 4; }
export { perimeter };
enum Color { Red, Green }
print(Color.Red);
"#;
    assert_eq!(rules(input, "no-unused-vars"), Vec::<String>::new());
}

#[test]
fn test_comments_may_exceed_the_line_length() {
    let long = "x".repeat(120);
    let input = format!("// {long}\nlet s = \"{long}\";\nprint(s);\n");
    assert_eq!(
        rules(&input, "max-line-length"),
        vec!["2:101 Line exceeds maximum line length of 100 characters"]
    );
}
