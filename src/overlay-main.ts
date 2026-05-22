import OverlayButton from "./lib/OverlayButton.svelte";
import { mount } from "svelte";

const app = mount(OverlayButton, {
  target: document.getElementById("app")!,
});

export default app;
