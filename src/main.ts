import { createApp } from "vue"
import { createPinia } from "pinia"
import App from "./App.vue"
import "./assets/main.css"

const blockZoom = (event: Event) => event.preventDefault()

window.addEventListener(
  "wheel",
  (event) => {
    if (event.ctrlKey || event.metaKey) event.preventDefault()
  },
  { capture: true, passive: false },
)
window.addEventListener("gesturestart", blockZoom, { passive: false })
window.addEventListener("gesturechange", blockZoom, { passive: false })
window.addEventListener("gestureend", blockZoom, { passive: false })
window.addEventListener("keydown", (event) => {
  if (
    (event.ctrlKey || event.metaKey) &&
    ["+", "-", "=", "0"].includes(event.key)
  ) {
    event.preventDefault()
  }
})

createApp(App).use(createPinia()).mount("#app")
