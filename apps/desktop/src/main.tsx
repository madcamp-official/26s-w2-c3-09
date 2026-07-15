import React from "react";
import ReactDOM from "react-dom/client";

import { AppShell } from "./AppShell";
import { CharacterOverlay } from "./features/overlay/CharacterOverlay";
import { ChatOverlay } from "./features/overlay/ChatOverlay";
import { HouseOverlay } from "./features/overlay/HouseOverlay";
import {
  isChatOverlayWindow,
  isCharacterOverlayWindow,
  isHouseOverlayWindow,
  isOverlayWindow
} from "./features/overlay/overlayApi";
import "./styles.css";

const characterOverlayWindow = isCharacterOverlayWindow();
const houseOverlayWindow = isHouseOverlayWindow();
const chatOverlayWindow = isChatOverlayWindow();

// The same bundle loads in both the main window and the overlay window; render by window label.
// Apply overlay body classes before React mounts to avoid a white first frame on transparent
// windows, while keeping house/chat/character styles isolated from each other.
if (isOverlayWindow()) {
  document.documentElement.classList.add("overlay-root");
  if (characterOverlayWindow) {
    document.body.classList.add("overlay-body");
  } else if (houseOverlayWindow) {
    document.body.classList.add("house-overlay-body");
  } else if (chatOverlayWindow) {
    document.body.classList.add("overlay-body", "chat-overlay-body");
  }
}
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
root.render(
  <React.StrictMode>
    {characterOverlayWindow ? (
      <CharacterOverlay />
    ) : chatOverlayWindow ? (
      <ChatOverlay />
    ) : houseOverlayWindow ? (
      <HouseOverlay />
    ) : (
      <AppShell />
    )}
  </React.StrictMode>
);
