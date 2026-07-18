import { useId, useState, type KeyboardEvent, type ReactNode } from 'react'

export interface TabItem {
  id: string
  label: string
  content: ReactNode
}

/**
 * A small, dependency-free, accessible tab group. Used for install channels and
 * cross-file fixture files. Follows the WAI-ARIA tabs pattern: roving arrow-key
 * focus, Home/End, and aria-controls wiring.
 */
export function Tabs({ tabs, className }: { tabs: TabItem[]; className?: string }) {
  const [active, setActive] = useState(0)
  const base = useId()

  function onKey(e: KeyboardEvent<HTMLButtonElement>) {
    let next = active
    if (e.key === 'ArrowRight') next = (active + 1) % tabs.length
    else if (e.key === 'ArrowLeft') next = (active - 1 + tabs.length) % tabs.length
    else if (e.key === 'Home') next = 0
    else if (e.key === 'End') next = tabs.length - 1
    else return
    e.preventDefault()
    setActive(next)
    document.getElementById(`${base}-tab-${next}`)?.focus()
  }

  return (
    <div className={`tabs${className ? ` ${className}` : ''}`}>
      <div className="tablist" role="tablist">
        {tabs.map((t, i) => (
          <button
            key={t.id}
            id={`${base}-tab-${i}`}
            type="button"
            role="tab"
            aria-selected={i === active}
            aria-controls={`${base}-panel-${i}`}
            tabIndex={i === active ? 0 : -1}
            className={`tab${i === active ? ' active' : ''}`}
            onClick={() => setActive(i)}
            onKeyDown={onKey}
          >
            {t.label}
          </button>
        ))}
      </div>
      {tabs.map((t, i) => (
        <div
          key={t.id}
          id={`${base}-panel-${i}`}
          role="tabpanel"
          aria-labelledby={`${base}-tab-${i}`}
          hidden={i !== active}
          className="tabpanel"
        >
          {i === active ? t.content : null}
        </div>
      ))}
    </div>
  )
}
