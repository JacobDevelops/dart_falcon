import { useState } from 'react'
import { Icon } from './Icon'

/** A one-line shell command with a copy button; the command stays selectable without JS. */
export function CopyCommand({ command }: { command: string }) {
  const [copied, setCopied] = useState(false)
  return (
    <div className="cmd">
      <code>
        <span className="cmd-prompt" aria-hidden="true">$ </span>
        {command}
      </code>
      <button
        type="button"
        className="cmd-copy"
        aria-label={copied ? 'Copied' : 'Copy command'}
        onClick={() =>
          navigator.clipboard.writeText(command).then(() => {
            setCopied(true)
            setTimeout(() => setCopied(false), 1600)
          })
        }
      >
        <Icon name={copied ? 'check' : 'copy'} />
      </button>
    </div>
  )
}
