import { mount } from 'svelte'
import '@fontsource/ibm-plex-sans/latin-400.css'
import '@fontsource/ibm-plex-sans/latin-600.css'
import '@fontsource/ibm-plex-mono/latin-400.css'
import '../../app.css'
import FractionalFlowPage from './FractionalFlowPage.svelte'

// A second entry point. It mounts its own root, shares the stylesheet, and knows nothing about
// App.svelte — see the component's header for why that independence is the point.
export default mount(FractionalFlowPage, {
  target: document.getElementById('app')!,
})
