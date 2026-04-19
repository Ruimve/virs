/* @refresh reload */
import { render } from 'solid-js/web'
import { Router } from '@solidjs/router'
import './index.css'
import App from './App'

const root = document.getElementById('root')

render(
  () => (
    <Router root={App}>
      {/* Routes are defined in App.tsx */}
    </Router>
  ),
  root!
)
