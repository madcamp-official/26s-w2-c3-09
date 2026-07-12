import React from "react";
import ReactDOM from "react-dom/client";

import { AppShell } from "./AppShell";
import { CharacterOverlay } from "./features/overlay/CharacterOverlay";
import { isOverlayWindow } from "./features/overlay/overlayApi";
import "./styles.css";

// The same bundle loads in both the main window and the overlay window; render by window label.
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
root.render(
  <React.StrictMode>{isOverlayWindow() ? <CharacterOverlay /> : <AppShell />}</React.StrictMode>
);
