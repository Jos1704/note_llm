import { invoke } from "@tauri-apps/api/core";

document.getElementById("notch")!.addEventListener("click", () => {
  invoke("mostrar_rapida").catch(() => {});
});
