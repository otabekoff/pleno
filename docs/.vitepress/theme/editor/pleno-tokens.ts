// The Pleno tokenizer, without CodeMirror: the editor wraps it in a
// StreamLanguage, and `highlight` uses it to color static code (the home page
// samples, and the editor's first paint before CodeMirror loads), so both
// look the same.

const KEYWORDS = new Set([
  'abstract', 'as', 'async', 'await', 'break', 'case', 'catch', 'class', 'const',
  'continue', 'deep', 'do', 'else', 'enum', 'export', 'extend', 'extends', 'finally',
  'fn', 'for', 'from', 'get', 'if', 'implements', 'import', 'in', 'interface',
  'internal', 'is', 'lazy', 'let', 'macro', 'match', 'module', 'move', 'mut', 'new',
  'operator', 'out', 'private', 'property', 'protected', 'public', 'return', 'set',
  'static', 'super', 'this', 'throw', 'try', 'type', 'while', 'constructor',
])
const ATOMS = new Set(['true', 'false', 'null'])

export interface TokenState {
  /** Inside a block comment that started on an earlier line. */
  inComment: boolean
  /** Inside a string that started on an earlier line: its delimiter. */
  inString: string | null
}

/** The part of CodeMirror's StringStream the tokenizer uses. */
export interface Stream {
  next(): string | void
  peek(): string | void
  eatSpace(): boolean
  skipToEnd(): void
  skipTo(text: string): boolean | void
  match(pattern: string | RegExp, consume?: boolean): boolean | RegExpMatchArray | null
}

/** Token names, as CodeMirror's StreamLanguage expects them. */
export type Token = 'keyword' | 'atom' | 'number' | 'string' | 'comment' | 'typeName' | 'variableName.function' | 'variableName' | 'meta' | null

function readString(stream: Stream, quote: string): boolean {
  let escaped = false
  let ch: string | void
  while ((ch = stream.next()) != null) {
    if (ch === quote && !escaped) return true
    escaped = !escaped && ch === '\\'
  }
  return false
}

export const plenoStreamParser = {
  name: 'pleno',
  startState: (): TokenState => ({ inComment: false, inString: null }),
  token(stream: Stream, state: TokenState): Token {
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
    if (ch === 'r' && stream.match(/^r"[^"]*"/)) return 'string'
    if (ch === "'") {
      if (!stream.match(/^'(\\.|[^'\\])'/)) stream.next()
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
}

/** A minimal StringStream over one line. */
class LineStream implements Stream {
  pos = 0
  constructor(readonly line: string) {}

  next() {
    if (this.pos < this.line.length) return this.line[this.pos++]
  }
  peek() {
    if (this.pos < this.line.length) return this.line[this.pos]
  }
  eatSpace() {
    const start = this.pos
    while (this.pos < this.line.length && /\s/.test(this.line[this.pos])) this.pos++
    return this.pos > start
  }
  skipToEnd() {
    this.pos = this.line.length
  }
  skipTo(text: string) {
    const found = this.line.indexOf(text, this.pos)
    if (found < 0) return false
    this.pos = found
    return true
  }
  match(pattern: string | RegExp, consume = true) {
    if (typeof pattern === 'string') {
      if (!this.line.startsWith(pattern, this.pos)) return false
      if (consume) this.pos += pattern.length
      return true
    }
    const found = this.line.slice(this.pos).match(pattern)
    if (!found || found.index !== 0) return null
    if (consume) this.pos += found[0].length
    return found
  }
}

const CLASS: Record<string, string> = {
  keyword: 'tok-keyword',
  atom: 'tok-number',
  number: 'tok-number',
  string: 'tok-string',
  comment: 'tok-comment',
  typeName: 'tok-type',
  'variableName.function': 'tok-function',
  meta: 'tok-keyword',
}

function escapeHtml(text: string) {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

/** Pleno code as HTML, one string per line, with `tok-*` classes on tokens. */
export function highlightLines(code: string): string[] {
  const state = plenoStreamParser.startState()
  return code.replace(/\n$/, '').split('\n').map((line) => {
    const stream = new LineStream(line)
    let html = ''
    while (stream.pos < line.length) {
      const start = stream.pos
      const token = plenoStreamParser.token(stream, state)
      if (stream.pos === start) stream.pos++ // never stall
      const text = escapeHtml(line.slice(start, stream.pos))
      const cls = token && CLASS[token]
      html += cls ? `<span class="${cls}">${text}</span>` : text
    }
    return html
  })
}

/** Pleno code as one HTML string. */
export function highlight(code: string): string {
  return highlightLines(code).join('\n')
}
