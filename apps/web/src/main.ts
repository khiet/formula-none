import { REVISION } from "three";

const app = document.querySelector<HTMLDivElement>("#app");
if (app) {
  app.textContent = `Formula None (three r${REVISION})`;
}
