import { mount } from 'svelte'
import './styles/fonts.css'
import './styles/tokens.css'
import './styles/cubby.css'
import './styles/inspector.css'
import './app.css'
import App from './App.svelte'

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
