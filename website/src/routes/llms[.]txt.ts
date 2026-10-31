import { createFileRoute } from '@tanstack/react-router'
import { llmsIndex, textResponse } from '../lib/agent-docs'

export const Route = createFileRoute('/llms.txt')({
  server: { handlers: { GET: () => textResponse(llmsIndex()) } },
})
