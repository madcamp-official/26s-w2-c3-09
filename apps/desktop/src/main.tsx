import React from "react";
import ReactDOM from "react-dom/client";

import { AgentPanel } from "./features/agent/AgentPanel";
import { FileEnginePanel } from "./features/files/FileEnginePanel";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <main className="app-shell">
      <AgentPanel />
      <FileEnginePanel embedded />
    </main>
  </React.StrictMode>
);
