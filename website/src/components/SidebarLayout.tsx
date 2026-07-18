import type { ReactNode } from 'react'

export function SidebarLayout({
  sidebar,
  children,
}: {
  sidebar: ReactNode
  children: ReactNode
}) {
  return (
    <div className="wrap">
      <div className="layout">
        <aside className="sidebar">{sidebar}</aside>
        <div className="content">{children}</div>
      </div>
    </div>
  )
}
