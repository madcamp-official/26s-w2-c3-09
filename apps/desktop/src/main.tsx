import React from "react";
import ReactDOM from "react-dom/client";

import { AppShell } from "./AppShell";
import { CharacterOverlay } from "./features/overlay/CharacterOverlay";
import { HouseOverlay } from "./features/overlay/HouseOverlay";
import { isCharacterOverlayWindow, isHouseOverlayWindow, isOverlayWindow } from "./features/overlay/overlayApi";
import "./styles.css";

// The same bundle loads in both the main window and the overlay window; render by window label.
const overlayWindow = isOverlayWindow();
if (overlayWindow) {
  document.documentElement.classList.add("overlay-root");
  document.body.classList.add("overlay-body");
}
const characterOverlayWindow = isCharacterOverlayWindow();
const houseOverlayWindow = isHouseOverlayWindow();
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
root.render(
  <React.StrictMode>
    {characterOverlayWindow ? <CharacterOverlay /> : houseOverlayWindow ? <HouseOverlay /> : <AppShell />}
  </React.StrictMode>
);
