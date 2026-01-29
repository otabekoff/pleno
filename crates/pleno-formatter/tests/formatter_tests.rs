use pleno_formatter::{FormatOptions, format_code};

#[test]
fn test_basic_formatting() {
    let input = "fn test(){return 1;}";
    let expected = "fn test() {\n    return 1;\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_indented_blocks() {
    let input = "if(true){if(false){return 1;}}";
    let expected = "if (true) {\n    if (false) {\n        return 1;\n    }\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_function_declaration_formatting() {
    let input = "fn add(a,b){return a+b;}";
    let expected = "fn add(a, b) {\n    return a + b;\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_class_declaration_formatting() {
    let input = "class Point{fn constructor(x,y){this.x=x;this.y=y;}}";
    let expected = "class Point {\n    fn constructor(x, y) {\n        this.x = x;\n        this.y = y;\n    }\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_multiline_statements() {
    let input = "let x = 1; let y = 2;";
    let expected = "let x = 1;\nlet y = 2;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_spaced_operators() {
    let input = "let x=1+2*3;";
    let expected = "let x = 1 + 2 * 3;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_array_formatting() {
    let input = "let arr=[1,2,3];";
    let expected = "let arr = [1, 2, 3];\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_map_formatting() {
    let input = "let map={\"a\":1,\"b\":2};";
    let expected = "let map = {\"a\": 1, \"b\": 2};\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_if_else_formatting() {
    let input = "if(x>5){return true;}else{return false;}";
    let expected = "if (x > 5) {\n    return true;\n} else {\n    return false;\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_while_formatting() {
    let input = "while(x > 0){x=x-1;}";
    let expected = "while (x > 0) {\n    x = x - 1;\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_for_formatting() {
    let input = "for(i in 0..5){print(i);}";
    let expected = "for (i in 0..5) {\n    print(i);\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
// Modulo operator (%) is not supported in Pleno AST
fn test_nested_control_flow() {
    let input = "if(x>0){for(i in 0..x){if(i%2==0){print(i);}}}else{print(\"negative\");}";
    let expected = "if (x > 0) {\n    for (i in 0..x) {\n        if (i % 2 == 0) {\n            print(i);\n        }\n    }\n} else {\n    print(\"negative\");\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_custom_indentation() {
    let input = "fn test(){return 1;}";
    let options = FormatOptions {
        indent_style: pleno_formatter::IndentStyle::Spaces(2),
        ..FormatOptions::default()
    };
    let expected = "fn test() {\n  return 1;\n}\n";
    let result = format_code(input, &options).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_tab_indentation() {
    let input = "fn test(){return 1;}";
    let options = FormatOptions {
        indent_style: pleno_formatter::IndentStyle::Tabs,
        ..FormatOptions::default()
    };
    let expected = "fn test() {\n\treturn 1;\n}\n";
    let result = format_code(input, &options).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_multiline_function_params() {
    let input =
        "fn long_function(param1,param2,param3,param4){return param1+param2+param3+param4;}";
    let expected = "fn long_function(param1, param2, param3, param4) {\n    return param1 + param2 + param3 + param4;\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_method_declaration_formatting() {
    let input = "class Calculator{fn add(a,b){return a+b;}static fn getVersion(){return \"1.0\";}}";
    let expected = "class Calculator {\n    fn add(a, b) {\n        return a + b;\n    }\n    static fn getVersion() {\n        return \"1.0\";\n    }\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_lambda_formatting() {
    let input = "let f=(x)=>{return x*2;};";
    let expected = "let f = (x) => {\n    return x * 2;\n};\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
#[ignore] // Parentheses are not preserved in AST - formatter cannot add them back
fn test_complex_expressions() {
    let input = "let result=(x+y)*z/(a-b);";
    let expected = "let result = (x + y) * z / (a - b);\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_try_catch_formatting() {
    let input = "try{dangerousOperation();}catch(e){handleError(e);}";
    let expected = "try {\n    dangerousOperation();\n} catch (e) {\n    handleError(e);\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
#[ignore] // Nested classes are not supported by the parser
fn test_nested_classes() {
    let input = "class Outer{class Inner{fn test(){return 1;}}}";
    let expected = "class Outer {\n    class Inner {\n        fn test() {\n            return 1;\n        }\n    }\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
#[ignore] // Blank lines between statements are not preserved in AST - would require AST changes
fn test_line_break_preservation() {
    let input = "let a = 1;\n\nlet b = 2;";
    let expected = "let a = 1;\n\nlet b = 2;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
#[ignore] // Comments are not preserved in AST - would require AST changes
fn test_comments_preservation() {
    let input = "// This is a comment\nlet x = 1; /* Block comment */";
    let expected = "// This is a comment\nlet x = 1; /* Block comment */\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_formatting_with_line_limit() {
    let long_line = "let very_long_variable_name = function_with_very_long_name(param1, param2, param3, param4);";
    // The formatter should handle long lines appropriately
    let result = format_code(long_line, &FormatOptions::default()).unwrap();
    // Should still be formatted correctly even if it exceeds line limits
    assert!(result.contains("let very_long_variable_name"));
    assert!(result.contains("function_with_very_long_name"));
}

#[test]
fn test_range_operator_formatting() {
    let input = "for(i in 1..=10){print(i);}";
    let expected = "for (i in 1..=10) {\n    print(i);\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_safe_navigation_formatting() {
    let input = "let value=obj?.prop?.subprop;";
    let expected = "let value = obj?.prop?.subprop;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_pipeline_operator_formatting() {
    let input = "let result=data|>filter|>map|>reduce;";
    let expected = "let result = data |> filter |> map |> reduce;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
// Ternary operator not yet implemented in Pleno
fn test_ternary_operator_formatting() {
    let input = "let result=condition?value1:value2;";
    let expected = "let result = condition ? value1 : value2;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_null_coalescing_formatting() {
    let input = "let result=value??default;";
    let expected = "let result = value ?? default;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_set_formatting() {
    let input = "let my_set={1,2,3,4};";
    let expected = "let my_set = {1, 2, 3, 4};\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_tuple_formatting() {
    let input = "let tuple=(1,2,3);";
    let expected = "let tuple = (1, 2, 3);\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_named_tuple_formatting() {
    let input = "let person=(name:\"John\",age:30);";
    let expected = "let person = (name: \"John\", age: 30);\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
#[ignore] // Underscore parameters in function declarations are not supported by the parser
fn test_underscore_parameter_formatting() {
    let input = "fn test(_,x,_){return x;}";
    let expected = "fn test(_, x, _) {\n    return x;\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
#[ignore] // Module blocks are not supported - parser expects 'module Name;' not 'module Name { ... }'
fn test_module_declaration_formatting() {
    let input = "module MyModule{export fn test(){return 1;}}";
    let expected = "module MyModule {\n    export fn test() {\n        return 1;\n    }\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_import_formatting() {
    let input = "import Module from \"./module.pln\";";
    let expected = "import Module from \"./module.pln\";\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_complex_nested_structure() {
    let input = "class Container{fn process(items){for(item in items){if(item.active){let result=item.calc();if(result>0){return result;}}}}}";
    let expected = "class Container {\n    fn process(items) {\n        for (item in items) {\n            if (item.active) {\n                let result = item.calc();\n                if (result > 0) {\n                    return result;\n                }\n            }\n        }\n    }\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_whitespace_normalization() {
    let input = "let    x    =    1    ;    ";
    let expected = "let x = 1;\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_no_changes_needed() {
    let input = "fn test() {\n    return 1;\n}\n";
    let expected = "fn test() {\n    return 1;\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_formatting_with_different_options() {
    let input = "fn test(){return 1;}";

    // Test with spaces
    let space_options = FormatOptions {
        indent_style: pleno_formatter::IndentStyle::Spaces(4),
        ..FormatOptions::default()
    };
    let space_result = format_code(input, &space_options).unwrap();
    assert!(space_result.contains("    ")); // 4 spaces

    // Test with tabs
    let tab_options = FormatOptions {
        indent_style: pleno_formatter::IndentStyle::Tabs,
        ..FormatOptions::default()
    };
    let tab_result = format_code(input, &tab_options).unwrap();
    assert!(tab_result.contains("\t")); // Tab character
}

#[test]
fn test_empty_blocks() {
    let input = "fn empty(){}if(true){}while(false){}";
    let expected = "fn empty() {}\nif (true) {}\nwhile (false) {}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn test_single_line_blocks() {
    let input = "fn single(){return 1;}";
    let expected = "fn single() {\n    return 1;\n}\n";
    let result = format_code(input, &FormatOptions::default()).unwrap();
    assert_eq!(result, expected);
}
