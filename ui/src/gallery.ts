// Entry for gallery.html (dev only).
import { mount } from 'svelte'
import './styles/fonts.css'
import './styles/tokens.css'
import './styles/cubby.css'
import Gallery from './lib/dev/Gallery.svelte'

const gallery = mount(Gallery, {
  target: document.getElementById('gallery')!,
})

export default gallery
