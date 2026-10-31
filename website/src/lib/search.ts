import { createServerFn } from '@tanstack/react-start'
import { SEARCH_INDEX } from './search-index.server'

export interface SearchEntry {
  id: string
  title: string
  description: string
  path: string
  hash: string
  kind: 'Rule' | 'Guide' | 'Section' | 'Config key' | 'Group' | 'Domain'
  aliases: string[]
}

// Called only when search opens. Full rule docs and examples never cross this boundary.
export const getSearchIndex = createServerFn({ method: 'GET' }).handler((): SearchEntry[] => SEARCH_INDEX)
