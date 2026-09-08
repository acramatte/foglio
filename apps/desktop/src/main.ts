import { api } from "./api";
import { App } from "./app";
import "./style.css";
const app = new App(document.querySelector<HTMLElement>("#app")!, api);
void app.start();
window.addEventListener("beforeunload", () => app.stop());
