import { useState } from 'react'
import { Icon } from './Icon'

/** A selectable snippet with a copy button. */
export function CopyCommand({
  command,
  prompt = true,
  label = 'Copy command',
}: {
  command: string
  prompt?: boolean
  label?: string
}) {
  const [copied, setCopied] = useState(false)
  return (
    <div className="cmd">
      <code>
        {prompt ? <span className="cmd-prompt" aria-hidden="true">$ </span> : null}
        {command}
      </code>
      <button
        type="button"
        className="cmd-copy"
        aria-label={copied ? 'Copied' : label}
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
