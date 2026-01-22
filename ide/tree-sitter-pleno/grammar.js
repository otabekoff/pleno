module.exports = grammar({
  name: 'pleno',

  extras: $ => [
    /\s/,
    $.comment,
  ],

  rules: {
    source_file: $ => repeat($._statement),

    _statement: $ => choice(
      $.let_statement,
      $.expression_statement,
    ),

    let_statement: $ => seq(
      'let',
      optional('mut'),
      field('name', $.identifier),
      optional(seq(':', field('type', $.identifier))),
      optional(seq('=', field('value', $._expression))),
      ';'
    ),

    expression_statement: $ => seq($._expression, ';'),

    _expression: $ => choice(
      $.binary_expression,
      $.unary_expression,
      $.literal,
      $.identifier,
      $.parenthesized_expression,
    ),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    unary_expression: $ => prec(3, seq(
      choice('!', '-'),
      $._expression
    )),

    binary_expression: $ => choice(
      prec.left(2, seq($._expression, choice('*', '/'), $._expression)),
      prec.left(1, seq($._expression, choice('+', '-'), $._expression)),
      prec.left(0, seq($._expression, choice('==', '!=', '<', '<=', '>', '>='), $._expression))
    ),

    literal: $ => choice(
      $.number,
      $.string,
      $.boolean,
      'null'
    ),

    number: $ => token(choice(
      /\d+\.\d+/, // float
      /\d+/
    )),

    string: $ => token(seq(
      '"',
      repeat(choice(/[^"\\]/, /\\./)),
      '"'
    )),

    boolean: $ => choice('true', 'false'),

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    comment: $ => token(choice(
      seq('//', /[^\n]*/),
      seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')
    ))
  }
});
