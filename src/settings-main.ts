import SettingsPage from "./lib/SettingsPage.svelte";
import { mount } from "svelte";

const app = mount(SettingsPage, {
  target: document.getElementById("app")!,
});

export default app;
