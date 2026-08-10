import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '../../design/theme.css'
import './index.css'
import './theme-counterparts.css'
import '../../design/effects.js'
import App from './App.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
