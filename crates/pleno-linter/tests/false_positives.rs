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

    assert!(
        !messages.iter().any(|msg| msg.rule_name == "no-unused-vars"),
        "Variables used in literals should be marked as used"
    );
}
