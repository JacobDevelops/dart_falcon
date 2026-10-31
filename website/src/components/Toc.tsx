import { useCallback, useEffect, useRef, useState } from 'react'
import { useLocation } from '@tanstack/react-router'
import { expandSection } from './CollapsibleSection'

export interface TocItem {
  id: string
  title: string
  level?: 2 | 3
}

export function expandTarget(id: string) {
  const target = document.getElementById(id)
  let expanded = false
  let ancestor = target
  while (ancestor) {
    if (ancestor.matches('.collapsible-section')) {
      const button = ancestor.querySelector<HTMLButtonElement>(':scope > h2 > .section-toggle')
      if (button?.getAttribute('aria-expanded') === 'false' && expandSection(ancestor)) {
        expanded = true
      }
    }
    if (ancestor instanceof HTMLDetailsElement && !ancestor.open) {
      ancestor.open = true
      expanded = true
    }
    ancestor = ancestor.parentElement
  }
  return { target, expanded }
}

export function Toc({ items }: { items: TocItem[] }) {
  const [activeId, setActiveId] = useState('')
  const hash = useLocation({ select: (location) => location.hash })
  const pathname = useLocation({ select: (location) => location.pathname })
  const expansionFrame = useRef(0)
  const idsKey = JSON.stringify(items.map(({ id }) => id))
  const selectTarget = useCallback((id: string) => {
    const { target, expanded } = expandTarget(id)
    if (!target) return
    setActiveId(id)
    if (expanded) {
      cancelAnimationFrame(expansionFrame.current)
      expansionFrame.current = requestAnimationFrame(() => target.scrollIntoView())
    }
  }, [])

  useEffect(() => {
    const followHash = (fragment: string) => {
      let id = fragment.replace(/^#/, '')
      try {
        id = decodeURIComponent(id)
      } catch {
        // External fragments may contain a literal percent sign.
      }
      if (id) selectTarget(id)
    }
    followHash(hash)
  }, [hash, pathname, idsKey, selectTarget])

  useEffect(() => () => cancelAnimationFrame(expansionFrame.current), [])

  useEffect(() => {
    const ids: string[] = JSON.parse(idsKey)
    const headings = ids.flatMap((id) => {
      const heading = document.getElementById(id)
      return heading ? [heading] : []
    })
    const updateCurrent = () => {
      const visible = headings.filter((heading) => heading.getClientRects().length > 0)
      const root = document.documentElement
      const padding = Number.parseFloat(getComputedStyle(root).scrollPaddingTop) || 0
      let current = visible[0]
      for (const heading of visible) {
        const margin = Number.parseFloat(getComputedStyle(heading).scrollMarginTop) || 0
        if (heading.getBoundingClientRect().top <= padding + margin + 2) current = heading
      }
      if (window.scrollY + root.clientHeight >= root.scrollHeight - 2) {
        current = visible[visible.length - 1]
      }
      setActiveId(current?.id ?? '')
    }
    let frame = 0
    const scheduleUpdate = () => {
      if (frame) return
      frame = requestAnimationFrame(() => {
        frame = 0
        updateCurrent()
      })
    }
    const observer = new ResizeObserver(scheduleUpdate)
    observer.observe(document.body)
    const content = document.querySelector('.content')
    if (content) observer.observe(content)
    const mutations = new MutationObserver(scheduleUpdate)
    if (content) mutations.observe(content, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['hidden', 'open'],
    })
    scheduleUpdate()
    window.addEventListener('scroll', scheduleUpdate, { passive: true })
    window.addEventListener('resize', scheduleUpdate)
    return () => {
      observer.disconnect()
      mutations.disconnect()
      cancelAnimationFrame(frame)
      window.removeEventListener('scroll', scheduleUpdate)
      window.removeEventListener('resize', scheduleUpdate)
    }
  }, [pathname, idsKey])

  if (!items.length) return null
  const sections: { item: TocItem; children: TocItem[] }[] = []
  for (const item of items) {
    const section = sections[sections.length - 1]
    if (item.level === 3 && section && section.item.level !== 3) {
      section.children.push(item)
    } else {
      sections.push({ item, children: [] })
    }
  }
  const renderLink = (item: TocItem) => (
    <a
      href={`#${item.id}`}
      aria-current={activeId === item.id ? 'location' : undefined}
      onClick={() => selectTarget(item.id)}
    >
      {item.title}
    </a>
  )
  return (
    <nav className="toc" aria-label="On this page">
      <p className="side-heading">On this page</p>
      <ul className="toc-list">
        {sections.map(({ item, children }) => (
          <li key={item.id}>
            {renderLink(item)}
            {children.length ? (
              <ul>
                {children.map((child) => <li key={child.id}>{renderLink(child)}</li>)}
              </ul>
            ) : null}
          </li>
        ))}
      </ul>
    </nav>
  )
}
