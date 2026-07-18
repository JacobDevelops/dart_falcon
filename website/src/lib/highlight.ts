// A tiny, dependency-free Dart highlighter. Produces themed HTML spans so the
// docs ship no syntax-highlighting runtime to the client.
const KEYWORDS = new Set([
  'abstract', 'as', 'assert', 'async', 'await', 'break', 'case', 'catch',
  'class', 'const', 'continue', 'covariant', 'default', 'deferred', 'do',
  'dynamic', 'else', 'enum', 'export', 'extends', 'extension', 'external',
  'factory', 'false', 'final', 'finally', 'for', 'get', 'hide', 'if',
  'implements', 'import', 'in', 'interface', 'is', 'late', 'library', 'mixin',
  'new', 'null', 'on', 'operator', 'part', 'required', 'rethrow', 'return',
  'sealed', 'set', 'show', 'static', 'super', 'switch', 'sync', 'this', 'throw',
  'true', 'try', 'typedef', 'var', 'void', 'while', 'with', 'yield',
])

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

export function highlightDart(code: string): string {
  const tokenRe =
    /(\/\/[^\n]*|\/\*[\s\S]*?\*\/)|(r?'''[\s\S]*?'''|r?"""[\s\S]*?"""|r?'(?:\\.|[^'\\\n])*'|r?"(?:\\.|[^"\\\n])*")|(@[A-Za-z_$][\w$]*)|(\b\d[\d_]*\.?\d*(?:[eE][+-]?\d+)?\b)|([A-Za-z_$][\w$]*)/g

  let out = ''
  let last = 0
  let m: RegExpExecArray | null
  while ((m = tokenRe.exec(code)) !== null) {
    out += escapeHtml(code.slice(last, m.index))
    const full = m[0]
    const comment = m[1]
    const str = m[2]
    const ann = m[3]
    const num = m[4]
    const ident = m[5]
    if (comment) out += `<span class="tok-c">${escapeHtml(comment)}</span>`
    else if (str) out += `<span class="tok-s">${escapeHtml(str)}</span>`
    else if (ann) out += `<span class="tok-a">${escapeHtml(ann)}</span>`
    else if (num) out += `<span class="tok-n">${escapeHtml(num)}</span>`
    else if (ident) {
      if (KEYWORDS.has(ident)) out += `<span class="tok-k">${escapeHtml(ident)}</span>`
      else if (/^[A-Z]/.test(ident)) out += `<span class="tok-t">${escapeHtml(ident)}</span>`
      else out += escapeHtml(ident)
    }
    last = m.index + full.length
  }
  out += escapeHtml(code.slice(last))
  return out
}

export function highlightJson(code: string): string {
  const re =
    /("(?:\\.|[^"\\])*")(\s*:)|("(?:\\.|[^"\\])*")|(\b(?:true|false|null)\b)|(-?\d+(?:\.\d+)?)/g
  let out = ''
  let last = 0
  let m: RegExpExecArray | null
  while ((m = re.exec(code)) !== null) {
    out += escapeHtml(code.slice(last, m.index))
    if (m[1]) out += `<span class="tok-k">${escapeHtml(m[1])}</span>${m[2] ?? ''}`
    else if (m[3]) out += `<span class="tok-s">${escapeHtml(m[3])}</span>`
    else if (m[4]) out += `<span class="tok-a">${escapeHtml(m[4])}</span>`
    else if (m[5]) out += `<span class="tok-n">${escapeHtml(m[5])}</span>`
    last = m.index + m[0].length
  }
  out += escapeHtml(code.slice(last))
  return out
}

export function highlight(code: string, lang: string): string {
  if (lang === 'json') return highlightJson(code)
  if (lang === 'dart') return highlightDart(code)
  return escapeHtml(code)
}
