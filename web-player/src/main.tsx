import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import VideoApp from './VideoApp.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <VideoApp />
  </StrictMode>,
)
