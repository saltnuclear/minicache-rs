export const metadata = {
  title: 'Mini-Cache Dashboard',
  description: 'Real-time monitoring for Mini-Cache',
}

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en">
      <body style={{ margin: 0, fontFamily: 'system-ui, -apple-system, sans-serif', backgroundColor: '#f5f5f5' }}>
        {children}
      </body>
    </html>
  )
}
