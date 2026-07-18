import { highlight } from '../lib/highlight'

interface Props {
  code: string
  filename?: string
  lang?: string
  className?: string
}

export function CodeBlock({ code, filename, lang = 'dart', className }: Props) {
  return (
    <div className={`codeblock${className ? ` ${className}` : ''}`}>
      {filename ? (
        <div className="codeblock-head">
          <span>{filename}</span>
          <span>{lang}</span>
        </div>
      ) : null}
      <pre>
        <code dangerouslySetInnerHTML={{ __html: highlight(code, lang) }} />
      </pre>
    </div>
  )
}
