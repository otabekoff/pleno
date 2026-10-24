// Pleno syntax for CodeMirror 6: a stream tokenizer with the keywords of
// crates/pleno-lexer, and colors taken from the --pl-tok-* CSS variables so
// the editor follows the light and dark themes.
import { HighlightStyle, StreamLanguage, syntaxHighlighting } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'

const KEYWORDS = new Set([
  'abstract', 'as', 'async', 'await', 'break', 'case', 'catch', 'class', 'const',
  'continue', 'deep', 'do', 'else', 'enum', 'export', 'extend', 'extends', 'finally',
  'fn', 'for', 'from', 'get', 'if', 'implements', 'import', 'in', 'interface',
  'internal', 'is', 'lazy', 'let', 'macro', 'match', 'module', 'move', 'mut', 'new',
  'operator', 'out', 'private', 'property', 'protected', 'public', 'return', 'set',
  'static', 'super', 'this', 'throw', 'try', 'type', 'while', 'constructor',
])
const ATOMS = new Set(['true', 'false', 'null'])

interface State {
  /** Inside a block comment that started on an earlier line. */
  inComment: boolean
  /** Inside a string that started on an earlier line: its delimiter. */
  inString: string | null
}

function readString(stream: { next(): string | null | void }, quote: string): boolean {
  let escaped = false
  let ch: string | null | void
  while ((ch = stream.next()) != null) {
    if (ch === quote && !escaped) return true
    escaped = !escaped && ch === '\\'
  }
  return false
}

export const plenoLanguage = StreamLanguage.define<State>({
  name: 'pleno',
  startState: () => ({ inComment: false, inString: null }),
  token(stream, state) {
    if (state.inComment) {
      if (stream.skipTo('*/')) {
        stream.match('*/')
        state.inComment = false
      } else {
        stream.skipToEnd()
      }
      return 'comment'
    }
    if (state.inString) {
      if (readString(stream, state.inString)) state.inString = null
      return 'string'
    }
    if (stream.eatSpace()) return null

    if (stream.match('//')) {
      stream.skipToEnd()
      return 'comment'
    }
    if (stream.match('/*')) {
      state.inComment = true
      return 'comment'
    }
    const ch = stream.peek()
    if (ch === '"' || ch === '`') {
      stream.next()
      if (!readString(stream, ch)) state.inString = ch
      return 'string'
    }
    if (ch === "'") {
      stream.match(/^'(\\.|[^'\\])'/) || stream.next()
      return 'string'
    }
    if (stream.match(/^\d[\d_]*(\.\d+)?([eE][+-]?\d+)?/)) return 'number'
    if (stream.match(/^@[A-Za-z_]\w*/)) return 'meta'

    const word = stream.match(/^[A-Za-z_]\w*/) as RegExpMatchArray | null
    if (word) {
      const text = word[0]
      if (KEYWORDS.has(text)) return 'keyword'
      if (ATOMS.has(text)) return 'atom'
      if (/^[A-Z]/.test(text)) return 'typeName'
      if (stream.peek() === '(') return 'variableName.function'
      return 'variableName'
    }
    stream.next()
    return null
  },
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
  { tag: t.comment, color: 'var(--pl-tok-comment)', fontStyle: 'italic' },
  { tag: t.typeName, color: 'var(--pl-tok-type)' },
  { tag: t.function(t.variableName), color: 'var(--pl-tok-function)' },
  { tag: t.meta, color: 'var(--pl-tok-keyword)' },
])

export const plenoHighlighting = syntaxHighlighting(plenoHighlightStyle)
