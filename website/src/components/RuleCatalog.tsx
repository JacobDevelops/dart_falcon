import { useEffect, useId, useMemo, useRef, useState } from 'react'
import { useLoaderData } from '@tanstack/react-router'
import { RuleRow } from './RuleRow'
import { CollapsibleSection } from './CollapsibleSection'
import { GROUPS, domainLabel, type RuleSummary } from '../lib/rules'

export interface CatalogSearch {
  q?: string
  group?: string
  domain?: string
  defaults?: string
  origin?: string
  crossFile?: string
}

export function catalogSearch(search: Partial<Record<keyof CatalogSearch, unknown>>, domains?: readonly string[]): CatalogSearch {
  const text = (key: keyof CatalogSearch) => typeof search[key] === 'string' ? search[key] : undefined
  return {
    q: text('q') || undefined,
    group: GROUPS.some((g) => g.id === search.group) ? text('group') : undefined,
    domain: !domains || domains.includes(String(search.domain)) ? text('domain') : undefined,
    defaults: ['recommended', 'opt-in'].includes(String(search.defaults)) ? text('defaults') : undefined,
    origin: ['official', 'other'].includes(String(search.origin)) ? text('origin') : undefined,
    crossFile: ['yes', 'no'].includes(String(search.crossFile)) ? text('crossFile') : undefined,
  }
}

export function RuleCatalog({ rules, search: searchInput, onChange, group, domain, overview = false }: {
  rules: RuleSummary[]
  search: CatalogSearch
  onChange: (search: CatalogSearch, replace?: boolean) => void
  group?: string
  domain?: string
  overview?: boolean
}) {
  const id = useId()
  const coverage = useLoaderData({ from: '__root__' })
  const domains = useMemo(() => coverage.domains.map((item) => item.name).sort(), [coverage.domains])
  const search = catalogSearch(searchInput, domains)
  const [draft, setDraft] = useState(search.q ?? '')
  const [moreFiltersOpen, setMoreFiltersOpen] = useState(Boolean(search.origin || search.crossFile))
  useEffect(() => {
    if (search.origin || search.crossFile) setMoreFiltersOpen(true)
  }, [search.origin, search.crossFile])
  const pendingQueries = useRef<string[]>([])
  useEffect(() => {
    const query = search.q ?? ''
    const pending = pendingQueries.current.indexOf(query)
    if (pending !== -1) {
      pendingQueries.current.splice(0, pending + 1)
      return
    }
    pendingQueries.current = []
    setDraft(query)
  }, [search.q])
  const query = (search.q ?? '').trim().toLowerCase().replace(/_/g, '-')
  const active = Object.entries(search).some(([key, value]) =>
    value && !(group && key === 'group') && !(domain && key === 'domain'),
  )
  const advancedCount = (search.origin ? 1 : 0) + (search.crossFile ? 1 : 0)
  const hits = rules.filter((rule) =>
    (!query || [rule.name, rule.upstreamId ?? '', rule.description].some((value) => value.toLowerCase().replace(/_/g, '-').includes(query))) &&
    (!(group ?? search.group) || rule.group === (group ?? search.group)) &&
    (!(domain ?? search.domain) || rule.domains.includes(domain ?? search.domain ?? '')) &&
    (!search.defaults || rule.recommended === (search.defaults === 'recommended')) &&
    (!search.origin || (rule.sourceKind === 'Lints') === (search.origin === 'official')) &&
    (!search.crossFile || rule.crossFile === (search.crossFile === 'yes')),
  )
  const update = (key: keyof CatalogSearch, value: string) => onChange({ ...search, q: draft || undefined, [key]: value || undefined })
  const select = (key: keyof CatalogSearch, label: string, options: { value: string; label: string }[]) => (
    <label htmlFor={`${id}-${key}`}>
      {label}
      <select id={`${id}-${key}`} value={search[key] ?? ''} onChange={(event) => update(key, event.target.value)}>
        <option value="">All</option>
        {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
      </select>
    </label>
  )
  return (
    <div className="catalog">
      <CollapsibleSection id="find-rules" title="Find a rule">
      <div className="search catalog-search">
        <label htmlFor={`${id}-query`}>Search IDs, Dart aliases, and descriptions</label>
        <input id={`${id}-query`} type="search" placeholder="avoid_print, unused-code, setState…" value={draft} onChange={(event) => {
          setDraft(event.target.value)
          pendingQueries.current.push(event.target.value)
          onChange({ ...search, q: event.target.value || undefined }, true)
        }} autoComplete="off" spellCheck={false} />
      </div>
      <div className="catalog-filters">
        {!group && select('group', 'Group', GROUPS.map((item) => ({ value: item.id, label: item.label })))}
        {!domain && select('domain', 'Domain', domains.map((name) => ({ value: name, label: domainLabel(name) })))}
        {select('defaults', 'Enabled by default', [{ value: 'recommended', label: 'Recommended' }, { value: 'opt-in', label: 'Opt-in' }])}
      </div>
      <details className="catalog-disclosure" open={moreFiltersOpen} onToggle={(event) => setMoreFiltersOpen(event.currentTarget.open)}>
        <summary>More filters{advancedCount ? ` (${advancedCount} active)` : ''}</summary>
        <div className="catalog-filters">
          {select('origin', 'Origin', [{ value: 'official', label: 'Official Dart lint' }, { value: 'other', label: 'falcon and others' }])}
          {select('crossFile', 'Cross-file analysis', [{ value: 'yes', label: 'Cross-file' }, { value: 'no', label: 'Single-file' }])}
        </div>
      </details>
      </CollapsibleSection>
      <CollapsibleSection id="matching-rules" title="Rules">
      <div className="catalog-status">
        <p className="search-count" role="status">{overview && !active ? 'Choose a group above, or search and filter the catalog.' : `${hits.length} ${hits.length === 1 ? 'rule' : 'rules'}`}</p>
        {active && <button type="button" className="catalog-clear" onClick={() => {
          pendingQueries.current = []
          setDraft('')
          onChange({})
        }}>Clear filters</button>}
      </div>
      {(!overview || active) && (hits.length ? <ul className="rule-list">{hits.map((rule) => <RuleRow key={rule.name} rule={rule} omitDomain={domain ?? search.domain} />)}</ul> : <p className="catalog-empty">No rules match these filters. Try a broader search or clear the filters.</p>)}
      </CollapsibleSection>
    </div>
  )
}
