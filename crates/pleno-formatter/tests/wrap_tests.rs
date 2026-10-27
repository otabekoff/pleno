//! Breaking long lines: a construct that fits stays on one line, one that does
//! not is broken one element per line, outermost first.

use pleno_formatter::{FormatOptions, format_code};

/// Formats with a short limit so the examples stay small, and checks that a
/// second format changes nothing.
fn fmt(source: &str, width: usize) -> String {
    let options = FormatOptions {
        max_line_length: Some(width),
        ..FormatOptions::default()
    };
    let once = format_code(source, &options).unwrap();
    let twice = format_code(&once, &options).unwrap();
    assert_eq!(once, twice, "formatting is not idempotent");
    once
}

#[test]
fn long_call_arguments_go_one_per_line() {
    let source = "check.equal(errorOf(() => c.r), \"cannot read internal field\");\n";
    assert_eq!(
        fmt(source, 40),
        "check.equal(\n    errorOf(() => c.r),\n    \"cannot read internal field\",\n);\n"
    );
}

#[test]
fn a_call_that_fits_stays_on_one_line() {
    let source = "check.equal(errorOf(() => c.r), \"cannot read\");\n";
    assert_eq!(fmt(source, 100), source);
}

#[test]
fn a_split_call_that_fits_is_joined() {
    let source = "check.equal(\n    short(1),\n    2\n);\n";
    assert_eq!(fmt(source, 100), "check.equal(short(1), 2);\n");
}

#[test]
fn a_split_call_that_does_not_fit_stays_split() {
    let source = "check.equal(\n    errorOf(() => c.r),\n    \"cannot read internal field 'r' of Circle outside its module\"\n);\n";
    assert_eq!(
        fmt(source, 80),
        "check.equal(\n    errorOf(() => c.r),\n    \"cannot read internal field 'r' of Circle outside its module\",\n);\n"
    );
}

#[test]
fn nested_calls_break_outermost_first() {
    let source = "let result = outer(inner(first, second), other(third, fourth));\n";
    assert_eq!(
        fmt(source, 40),
        "let result = outer(\n    inner(first, second),\n    other(third, fourth),\n);\n"
    );
    // An argument that is still too long is broken in turn.
    assert_eq!(
        fmt(source, 24),
        "let result = outer(\n    inner(\n        first,\n        second,\n    ),\n    other(\n        third,\n        fourth,\n    ),\n);\n"
    );
}

#[test]
fn long_arrays_maps_sets_and_tuples_break() {
    assert_eq!(
        fmt("let numbers = [100, 200, 300, 400, 500];\n", 30),
        "let numbers = [\n    100,\n    200,\n    300,\n    400,\n    500,\n];\n"
    );
    assert_eq!(
        fmt(
            "let config = {\"host\": \"localhost\", \"port\": 8080};\n",
            30
        ),
        "let config = {\n    \"host\": \"localhost\",\n    \"port\": 8080,\n};\n"
    );
    assert_eq!(
        fmt("let s = {first, second, third};\n", 20),
        "let s = {\n    first,\n    second,\n    third,\n};\n"
    );
    assert_eq!(
        fmt("let t = (first, second, third);\n", 20),
        "let t = (\n    first,\n    second,\n    third,\n);\n"
    );
    assert_eq!(
        fmt("let p = (name: first, age: second);\n", 20),
        "let p = (\n    name: first,\n    age: second,\n);\n"
    );
}

#[test]
fn long_binary_chains_break_before_the_operator() {
    let source = "let total = firstValue + secondValue + thirdValue;\n";
    assert_eq!(
        fmt(source, 30),
        "let total = firstValue\n    + secondValue\n    + thirdValue;\n"
    );
    // Only the outermost chain breaks while the rest fits.
    let source = "if (isReady(value) && isValid(value) || fallback) {\n    go();\n}\n";
    assert_eq!(
        fmt(source, 40),
        "if (isReady(value) && isValid(value)\n    || fallback) {\n    go();\n}\n"
    );
}

#[test]
fn method_chains_go_one_call_per_line() {
    let source = "let out = items.filter((x) => x.active).map((x) => x.value).take(10);\n";
    assert_eq!(
        fmt(source, 50),
        "let out = items\n    .filter((x) => x.active)\n    .map((x) => x.value)\n    .take(10);\n"
    );
    // A single method call breaks its arguments instead.
    assert_eq!(
        fmt("let out = this.items.filter(firstArgument, second);\n", 40),
        "let out = this.items.filter(\n    firstArgument,\n    second,\n);\n"
    );
}

#[test]
fn ternaries_break_before_question_mark_and_colon() {
    assert_eq!(
        fmt(
            "let pick = isReady(value) ? firstChoice : secondChoice;\n",
            40
        ),
        "let pick = isReady(value)\n    ? firstChoice\n    : secondChoice;\n"
    );
}

#[test]
fn long_parameter_lists_break() {
    let source =
        "fn describe(width: Int, height: Int, label: String): String {\n    return label;\n}\n";
    assert_eq!(
        fmt(source, 40),
        "fn describe(\n    width: Int,\n    height: Int,\n    label: String,\n): String {\n    return label;\n}\n"
    );
    let source = "class Shape {\n    fn area(width: Int, height: Int): Int {\n        return width;\n    }\n}\n";
    assert_eq!(
        fmt(source, 30),
        "class Shape {\n    fn area(\n        width: Int,\n        height: Int,\n    ): Int {\n        return width;\n    }\n}\n"
    );
}

#[test]
fn no_comma_follows_a_variadic_parameter() {
    let source = "fn collect(first: Int, ...rest) {\n    return first;\n}\n";
    assert_eq!(
        fmt(source, 20),
        "fn collect(\n    first: Int,\n    ...rest\n) {\n    return first;\n}\n"
    );
}

#[test]
fn long_import_and_export_lists_break() {
    let source = "import { alpha, beta as b, gamma } from \"./letters.pln\";\n";
    assert_eq!(
        fmt(source, 40),
        "import {\n    alpha,\n    beta as b,\n    gamma,\n} from \"./letters.pln\";\n"
    );
    assert_eq!(fmt(source, 100), source);
    assert_eq!(
        fmt("export { alpha, beta, gamma };\n", 20),
        "export {\n    alpha,\n    beta,\n    gamma,\n};\n"
    );
}

#[test]
fn comments_in_a_broken_list_stay_with_their_element() {
    let source =
        "check.equal(\n    first, // the first one\n    second\n    // after the last\n);\n";
    assert_eq!(
        fmt(source, 100),
        "check.equal(\n    first, // the first one\n    second,\n    // after the last\n);\n"
    );
    let source = "let arr = [\n    1, // one\n    // before three\n    3\n];\n";
    assert_eq!(
        fmt(source, 100),
        "let arr = [\n    1, // one\n    // before three\n    3,\n];\n"
    );
    // A comment that was on a short call keeps the call broken.
    assert_eq!(
        fmt("let x = foo(a, // first\n    b);\n", 100),
        "let x = foo(\n    a, // first\n    b,\n);\n"
    );
}

#[test]
fn comments_in_chains_and_match_arms_stay_in_place() {
    let source =
        "let chained = list\n    .filter((x) => x > 0) // positives\n    .map((x) => x * 2);\n";
    assert_eq!(fmt(source, 100), source);
    let source = "let sum = a\n    + b // plus b\n    + c;\n";
    assert_eq!(fmt(source, 100), source);
    let source = "match (x) {\n    // first arm\n    case 1 => print(\"one\"), // one\n    case _ => print(\"other\"),\n}\n";
    assert_eq!(fmt(source, 100), source);
}

#[test]
fn comments_inside_an_index_stay_in_the_brackets() {
    // A block comment stays on the line, next to the index.
    assert_eq!(
        fmt("let x = arr[ /* c */ 1 ];\n", 100),
        "let x = arr[/* c */ 1];\n"
    );
    assert_eq!(
        fmt("let x = arr[i /* after */];\n", 100),
        "let x = arr[i /* after */];\n"
    );
    assert_eq!(
        fmt("let q = m?.[ /* key */ \"k\" ];\n", 100),
        "let q = m?.[/* key */ \"k\"];\n"
    );
    // A line comment puts the index on a line of its own.
    assert_eq!(
        fmt("let y = arr[1 // c\n];\n", 100),
        "let y = arr[\n    1 // c\n];\n"
    );
    assert_eq!(
        fmt("let p = f(arr[1 // first\n], 2);\n", 100),
        "let p = f(\n    arr[\n        1 // first\n    ],\n    2,\n);\n"
    );
    assert_eq!(
        fmt(
            "let r = items.filter((i) => i > 0)[ /* first */ 0 ].name;\n",
            100
        ),
        "let r = items.filter((i) => i > 0)[/* first */ 0].name;\n"
    );
}

#[test]
fn block_comments_in_slices_parentheses_and_unary_operators_stay() {
    assert_eq!(
        fmt("let a = xs[1: /* to */ 2];\n", 100),
        "let a = xs[1:/* to */ 2];\n"
    );
    assert_eq!(
        fmt("let a = xs[/* from */ 1:2 /* end */];\n", 100),
        "let a = xs[/* from */ 1:2 /* end */];\n"
    );
    assert_eq!(
        fmt("let c = (/* paren */ 1 + 2 /* sum */);\n", 100),
        "let c = (/* paren */ 1 + 2 /* sum */);\n"
    );
    assert_eq!(
        fmt("let e = - /* neg */ 1;\n", 100),
        "let e = -/* neg */ 1;\n"
    );
}

#[test]
fn comments_inside_import_and_export_lists_stay_with_their_name() {
    assert_eq!(
        fmt("import { a, /* the b */ b } from \"./m\";\n", 100),
        "import {\n    a,\n    /* the b */ b,\n} from \"./m\";\n"
    );
    assert_eq!(
        fmt("import { a as x /* the x */, b } from \"./m\";\n", 100),
        "import {\n    a as x, /* the x */\n    b,\n} from \"./m\";\n"
    );
    let source = "import {\n    c, // the c\n    d,\n} from \"./n\";\n";
    assert_eq!(fmt(source, 100), source);
    assert_eq!(
        fmt("import {\n    c, // the c\n    d\n} from \"./n\";\n", 100),
        source
    );
    assert_eq!(
        fmt("export { x, /* exported */ b };\n", 100),
        "export {\n    x,\n    /* exported */ b,\n};\n"
    );
    // A name that also appears in a comment is found where it is written.
    assert_eq!(
        fmt("import { b /* not a */, a } from \"./m\";\n", 100),
        "import {\n    b, /* not a */\n    a,\n} from \"./m\";\n"
    );
}

#[test]
fn strings_and_comments_are_never_split() {
    let long = "\"a string that is much longer than the limit allows\"";
    let source = format!("print({long});\n");
    assert_eq!(fmt(&source, 20), format!("print(\n    {long},\n);\n"));
    let source = "let x = 1; // a comment that is much longer than the limit\n";
    assert_eq!(fmt(source, 20), source);
}

#[test]
fn lambda_bodies_are_laid_out_at_their_own_indent() {
    let source = "items.forEach((item) => {\n    print(describe(item.first, item.second, item.third));\n});\n";
    assert_eq!(
        fmt(source, 40),
        "items.forEach((item) => {\n    print(\n        describe(\n            item.first,\n            item.second,\n            item.third,\n        ),\n    );\n});\n"
    );
}

#[test]
fn else_if_stays_a_chain() {
    let source = "if (a) {\n    x();\n} else if (b) {\n    y();\n} else {\n    z();\n}\n";
    assert_eq!(fmt(source, 100), source);
}

#[test]
fn no_limit_never_breaks() {
    let options = FormatOptions {
        max_line_length: None,
        ..FormatOptions::default()
    };
    let source = format!("let x = f({});\n", vec!["argument"; 30].join(", "));
    assert_eq!(format_code(&source, &options).unwrap(), source);
}

#[test]
fn the_default_limit_is_100() {
    let fits = format!("let x = f({});\n", "a".repeat(100 - "let x = f();".len()));
    assert_eq!(fmt(&fits, 100), fits);
    let options = FormatOptions::default();
    assert_eq!(format_code(&fits, &options).unwrap(), fits);
    let too_long = format!(
        "let x = f({}, b);\n",
        "a".repeat(100 - "let x = f();".len())
    );
    assert!(format_code(&too_long, &options).unwrap().contains("f(\n"));
}
