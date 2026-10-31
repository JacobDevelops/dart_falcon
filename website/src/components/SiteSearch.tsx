import { useEffect, useMemo, useRef, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { getSearchIndex, type SearchEntry } from '../lib/search'
import { expandTarget } from './Toc'
import { Described } from './RuleRow'

function rank(entry: SearchEntry, query: string): number {
  const id = entry.title.toLowerCase()
  const aliases = entry.aliases.map((alias) => alias.toLowerCase())
  if (id === query || aliases.includes(query)) return 0
  if (id.startsWith(query) || aliases.some((alias) => alias.startsWith(query))) return 1
  if (id.includes(query) || aliases.some((alias) => alias.includes(query))) return 2
  const text = `${id} ${entry.description.toLowerCase()} ${aliases.join(' ')}`
  return query.split(/\s+/).every((word) => text.includes(word)) ? 3 : -1
}

export default function SiteSearch({ open, onClose }: { open: boolean, onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null)
  const input = useRef<HTMLInputElement>(null)
  const list = useRef<HTMLDivElement>(null)
  const returnFocus = useRef<HTMLElement | null>(null)
  const [index, setIndex] = useState<SearchEntry[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)

  useEffect(() => {
    if (!open) return
    const element = dialog.current
    if (!element) return
    const previous = document.activeElement
    if (previous instanceof HTMLElement && previous !== document.body && !element.contains(previous)) {
      returnFocus.current = previous
    } else if (!returnFocus.current?.isConnected) {
      returnFocus.current = document.querySelector<HTMLElement>('.nav-search')
    }
    element.showModal()
    input.current?.focus()
    return () => {
      element.close()
      requestAnimationFrame(() => {
        if (!element.open && returnFocus.current?.isConnected) {
          returnFocus.current.focus({ preventScroll: true })
        }
      })
    }
  }, [open])

  useEffect(() => {
    if (!open || index) return
    let cancelled = false
    setLoading(true)
    setError(false)
    getSearchIndex()
      .then((entries) => { if (!cancelled) setIndex(entries) })
      .catch(() => { if (!cancelled) setError(true) })
      .finally(() => { if (!cancelled) setLoading(false) })
    return () => { cancelled = true }
  }, [open, index, attempt])

  const results = useMemo(() => {
    if (!index) return []
    const normalized = query.trim().toLowerCase()
    if (!normalized) return index.filter((entry) => entry.kind === 'Guide')
    return index
      .map((entry) => ({ entry, score: rank(entry, normalized) }))
      .filter(({ score }) => score >= 0)
      .sort((a, b) => a.score - b.score || a.entry.title.localeCompare(b.entry.title))
      .slice(0, 30)
      .map(({ entry }) => entry)
  }, [index, query])
  const selected = Math.min(active, Math.max(0, results.length - 1))

  useEffect(() => {
    list.current?.querySelector(`[data-index="${selected}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [selected, query])
  let status = 'Start with a guide, or type a rule id or keyword.'
  if (error) status = 'Search could not load. Please try again.'
  else if (loading || !index) status = 'Loading search…'
  else if (!results.length) status = `No results for “${query.trim()}”. Try a rule id or a broader term.`
  else if (query.trim()) status = `${results.length === 30 ? 'First 30' : results.length} ${results.length === 1 ? 'result' : 'results'}`

  return (
    <dialog
      ref={dialog}
      className="site-search"
      aria-labelledby="site-search-title"
      onClose={(event) => { if (!event.currentTarget.open) onClose() }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault()
          onClose()
        }
      }}
      onClick={(event) => { if (event.target === event.currentTarget) onClose() }}
    >
      <div className="site-search-panel">
        <div className="site-search-header">
          <h2 id="site-search-title">Search falcon</h2>
          <button type="button" className="site-search-close" onClick={onClose} aria-label="Close search">
            Esc
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg>
          </button>
        </div>
        <label className="visually-hidden" htmlFor="site-search-input">Search rules and documentation</label>
        <input
          ref={input}
          id="site-search-input"
          className="site-search-input"
          type="search"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={open}
          aria-controls="site-search-results"
          aria-activedescendant={results.length ? `site-search-result-${selected}` : undefined}
          autoComplete="off"
          placeholder="Search rules, guides, and sections…"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value)
            setActive(0)
          }}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing || !results.length) return
            if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
              event.preventDefault()
              setActive((selected + (event.key === 'ArrowDown' ? 1 : -1) + results.length) % results.length)
            } else if (event.key === 'Enter') {
              event.preventDefault()
              list.current?.querySelector<HTMLAnchorElement>(`[data-index="${selected}"]`)?.click()
            }
          }}
        />
        <p className="site-search-status" role="status" aria-live="polite">
          {status}
        </p>
        {error && <button type="button" className="btn btn-ghost site-search-retry" onClick={() => setAttempt((value) => value + 1)}>Try again</button>}
        <div ref={list} className="site-search-results" id="site-search-results" role="listbox" aria-label="Search results" aria-busy={loading}>
          {results.map((result, position) => (
            <Link
              key={result.id}
              to={result.path}
              hash={result.hash || undefined}
              id={`site-search-result-${position}`}
              data-index={position}
              className="site-search-result"
              role="option"
              aria-selected={position === selected}
              tabIndex={-1}
              onMouseDown={(event) => event.preventDefault()}
              onMouseEnter={() => setActive(position)}
              onClick={() => {
                if (result.hash && result.path === window.location.pathname) {
                  const { target, expanded } = expandTarget(result.hash)
                  if (expanded) requestAnimationFrame(() => target?.scrollIntoView())
                }
                onClose()
              }}
            >
              <span className="site-search-result-heading"><strong>{result.title}</strong><span className="site-search-kind">{result.kind}</span></span>
              <span className="site-search-description"><Described text={result.description} /></span>
            </Link>
          ))}
        </div>
        <p className="site-search-help">↑ ↓ to choose · Enter to open · Esc to close</p>
      </div>
    </dialog>
  )
}
