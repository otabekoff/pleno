// Pleno for CodeMirror 6: the shared tokenizer as a StreamLanguage, colored
// from the --pl-tok-* CSS variables so the editor follows the light and dark
// themes and matches the static code on the page.
import { HighlightStyle, StreamLanguage, syntaxHighlighting } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'
import { plenoStreamParser, type TokenState } from './pleno-tokens'

export const plenoLanguage = StreamLanguage.define<TokenState>({
  ...plenoStreamParser,
  languageData: {
    commentTokens: { line: '//', block: { open: '/*', close: '*/' } },
    closeBrackets: { brackets: ['(', '[', '{', '"', '`'] },
  },
})

const plenoHighlightStyle = HighlightStyle.define([
  { tag: t.keyword, color: 'var(--pl-tok-keyword)' },
  { tag: t.atom, color: 'var(--pl-tok-number)' },
  { tag: t.number, color: 'var(--pl-tok-number)' },
  { tag: t.string, color: 'var(--pl-tok-string)' },
  { tag: t.comment, color: 'var(--pl-tok-comment)' },
  { tag: t.typeName, color: 'var(--pl-tok-type)' },
  { tag: t.function(t.variableName), color: 'var(--pl-tok-function)' },
  { tag: t.meta, color: 'var(--pl-tok-keyword)' },
])

export const plenoHighlighting = syntaxHighlighting(plenoHighlightStyle)
