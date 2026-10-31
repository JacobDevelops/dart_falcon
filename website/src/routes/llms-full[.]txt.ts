import { createFileRoute } from '@tanstack/react-router'
import { llmsFull, textResponse } from '../lib/agent-docs'

export const Route = createFileRoute('/llms-full.txt')({
  server: { handlers: { GET: () => textResponse(llmsFull()) } },
})
