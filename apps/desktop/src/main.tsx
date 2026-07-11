import React from "react";
import ReactDOM from "react-dom/client";

import { FileEnginePanel } from "./features/files/FileEnginePanel";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <FileEnginePanel />
  </React.StrictMode>
);
