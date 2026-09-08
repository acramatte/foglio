import { api } from "./api";
import { App } from "./app";
import "./style.css";
import { listen } from "@tauri-apps/api/event";
const app = new App(document.querySelector<HTMLElement>("#app")!, api);
// Subscribe before loading notes: a native close stays blocked if setup fails.
void listen("close-requested", () => { void app.requestClose(); }).then(() => app.start());
window.addEventListener("beforeunload", event => {
  if (app.hasUnsavedChanges) {event.preventDefault();event.returnValue="";}
});
window.addEventListener("unload", () => app.stop());
