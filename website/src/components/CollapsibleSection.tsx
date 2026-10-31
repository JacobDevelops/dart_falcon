import { useCallback, useState, type ReactNode } from 'react'

const sectionExpanders = new WeakMap<Element, () => void>()

export function expandSection(section: Element): boolean {
  const expand = sectionExpanders.get(section)
  if (!expand) return false
  expand()
  return true
}

export function CollapsibleSection({
  id,
  title,
  children,
  className = '',
  defaultExpanded = true,
}: {
  id: string
  title: ReactNode
  children: ReactNode
  className?: string
  defaultExpanded?: boolean
}) {
  const [expanded, setExpanded] = useState(defaultExpanded)
  const registerSection = useCallback((section: HTMLElement | null) => {
    if (!section) return
    sectionExpanders.set(section, () => setExpanded(true))
    return () => { sectionExpanders.delete(section) }
  }, [])
  return (
    <section ref={registerSection} className={`collapsible-section ${className}`.trim()}>
      <h2 id={id} className="section-heading">
        <button
          type="button"
          className="section-toggle"
          aria-expanded={expanded}
          aria-controls={`${id}-content`}
          onClick={() => setExpanded((value) => !value)}
        >
          {title}
        </button>
      </h2>
      <div id={`${id}-content`} className="section-content" hidden={!expanded}>{children}</div>
    </section>
  )
}
