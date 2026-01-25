import parser from "@typescript-eslint/parser";
import plugin from "@typescript-eslint/eslint-plugin";

export default [
    {
        files: ["**/*.ts"],
        languageOptions: {
            parser: parser,
            parserOptions: {
                sourceType: "module"
            }
        },
        plugins: {
            "@typescript-eslint": plugin
        },
        rules: {
            "@typescript-eslint/no-unused-vars": ["warn", { "argsIgnorePattern": "^_" }],
            "@typescript-eslint/no-explicit-any": "off"
        }
    }
];
