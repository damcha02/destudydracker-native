import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '../../design/theme.css'
import './index.css'
import './theme-counterparts.css'
import '../../design/effects.js'
import App from './App.tsx'
import { captureBootSnapshot } from './lib/releaseNotes'

// Before React mounts and the app's own debounced first save can create its storage keys: what is
// read here is what tells a first-ever run from an upgrade. See BootSnapshot.
captureBootSnapshot()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
