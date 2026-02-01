/*
Language: Pleno
Description: Pleno is a modern, high-performance programming language.
Author: Otabek <otabek@pleno.dev>
Website: https://pleno.dev
Category: common
*/

function pleno(hljs) {
  const KEYWORDS = {
    keyword:
      'let const mut fn return if else for while match class interface enum import from as export module ' +
      'case do label break continue try catch finally throw abstract implements extends constructor ' +
      'property get set static override label typeof extend operator macro lazy move type where ' +
      'yield implicit explicit init deinit del global nonlocal struct union is in new',
    literal:
      'true false null this super',
    type:
      'Int Float String Bool Char Void Any Null Map Set Promise Result Option Self self'
  };

  const COMMENT = hljs.COMMENT(
    '//',
    '$',
    {
      contains: [
        {
          className: 'doctag',
          begin: '///'
        }
      ]
    }
  );

  const STRING = {
    className: 'string',
    begin: '"', end: '"',
    illegal: '\\n',
    contains: [hljs.BACKSLASH_ESCAPE]
  };

  const NUMBER = {
    className: 'number',
    variants: [
      { begin: '\\b\\d+\\.\\d+\\b' },
      { begin: '\\b\\d+\\b' }
    ],
    relevance: 0
  };

  const PARAMS = {
    className: 'params',
    begin: /\(/, end: /\)/,
    keywords: KEYWORDS,
    contains: [
        hljs.SELF,
        hljs.C_BLOCK_COMMENT_MODE,
        STRING,
        NUMBER
    ]
  };

  return {
    name: 'Pleno',
    aliases: ['pln'],
    keywords: KEYWORDS,
    contains: [
      hljs.C_LINE_COMMENT_MODE,
      hljs.C_BLOCK_COMMENT_MODE,
      COMMENT,
      STRING,
      NUMBER,
      {
        className: 'function',
        beginKeywords: 'fn', end: /\{/, excludeEnd: true,
        contains: [
          hljs.UNDERSCORE_TITLE_MODE,
          PARAMS
        ]
      },
      {
        className: 'class',
        beginKeywords: 'class interface enum struct union', end: /\{/, excludeEnd: true,
        illegal: /[:"\[\]]/,
        contains: [
          { beginKeywords: 'extends implements' },
          hljs.UNDERSCORE_TITLE_MODE
        ]
      },
      {
        className: 'meta',
        begin: '@[a-zA-Z_][a-zA-Z0-9_]*'
      },
      {
        begin: /=>/ // Fat arrow
      }
    ]
  };
}

module.exports = pleno;
