import { useEffect, useMemo, useState } from "react";

import {
  analyzeRoot,
  executeFileChanges,
  listManagedRoots,
  ManagedRoot,
  precheckFileChanges,
  Proposal,
  ProposalReport,
  proposeFileChanges,
  registerManagedRoot,
  undoLastFileOperation
} from "./fileEngineApi";

type DecisionState = Record<string, "approved" | "rejected" | "pending">;

const demoRootPath =
  "C:\\Users\\user\\2026-project\\kaist_madcamp\\week2\\test-fixtures\\file-trees\\ui-demo";

export function FileEnginePanel() {
  const [pathInput, setPathInput] = useState("");
  const [roots, setRoots] = useState<ManagedRoot[]>([]);
  const [selectedRootId, setSelectedRootId] = useState("");
  const [proposal, setProposal] = useState<ProposalReport | null>(null);
  const [decisions, setDecisions] = useState<DecisionState>({});
  const [status, setStatus] = useState("Ready");
  const [error, setError] = useState<string | null>(null);
  const [resultLines, setResultLines] = useState<string[]>([]);

  const selectedRoot = roots.find((root) => root.root_id === selectedRootId);
  const approvedDecisions = useMemo(
    () =>
      Object.entries(decisions)
        .filter(([, decision]) => decision === "approved")
        .map(([proposal_id]) => ({ proposal_id, decision: "approved" as const })),
    [decisions]
  );

  useEffect(() => {
    void refreshRoots();
  }, []);

  async function refreshRoots() {
    setError(null);
    try {
      const storedRoots = await listManagedRoots();
      setRoots(storedRoots);
      setSelectedRootId((current) => current || storedRoots[0]?.root_id || "");
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }

  async function registerRoot() {
    setError(null);
    setStatus("Registering root");
    try {
      const managed = await registerManagedRoot(pathInput.trim());
      const storedRoots = await listManagedRoots();
      setRoots(storedRoots);
      setSelectedRootId(managed.root_id);
      setStatus("Root registered");
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Register failed");
    }
  }

  async function analyzeSelectedRoot() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Analyzing");
    try {
      const report = await analyzeRoot(selectedRootId);
      setResultLines(report.files.map((file) => `${file.path} (${file.size_bytes} bytes)`));
      setStatus(`Analyzed ${report.files.length} files`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Analyze failed");
    }
  }

  async function proposeForSelectedRoot() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Building proposal");
    try {
      const report = await proposeFileChanges(selectedRootId);
      setProposal(report);
      setDecisions(
        Object.fromEntries(
          report.proposals.map((item) => [
            item.proposal_id,
            item.status === "ready" ? "approved" : "pending"
          ])
        )
      );
      setResultLines(report.proposals.map(formatProposal));
      setStatus(`Prepared ${report.proposals.length} proposals`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Proposal failed");
    }
  }

  async function precheckSelectedRoot() {
    if (!selectedRootId || !proposal) return;

    setError(null);
    setStatus("Prechecking");
    try {
      const report = await precheckFileChanges(selectedRootId, proposal, approvedDecisions);
      setResultLines(
        report.checks.map((check) =>
          [check.status, `${check.from} -> ${check.to}`, check.reason].filter(Boolean).join(" | ")
        )
      );
      setStatus(`Prechecked ${report.checks.length} approved proposals`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Precheck failed");
    }
  }

  async function executeSelectedRoot() {
    if (!selectedRootId || !proposal) return;

    setError(null);
    setStatus("Executing");
    try {
      const report = await executeFileChanges(selectedRootId, proposal, approvedDecisions);
      setResultLines(
        report.results.map((result) =>
          [result.status, `${result.from} -> ${result.to}`, result.reason]
            .filter(Boolean)
            .join(" | ")
        )
      );
      setStatus(
        `Executed ${report.executed_count}, skipped ${report.skipped_count}, rejected ${report.rejected_count}`
      );
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Execute failed");
    }
  }

  async function undoSelectedRoot() {
    if (!selectedRootId) return;

    setError(null);
    setStatus("Undoing");
    try {
      const report = await undoLastFileOperation(selectedRootId);
      setResultLines(
        report.results.map((result) =>
          [result.status, `${result.from} -> ${result.to}`, result.reason]
            .filter(Boolean)
            .join(" | ")
        )
      );
      setStatus(`Undone ${report.undone_count}, skipped ${report.skipped_count}`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Undo failed");
    }
  }

  function setDecision(item: Proposal, decision: DecisionState[string]) {
    setDecisions((current) => ({ ...current, [item.proposal_id]: decision }));
  }

  return (
    <main className="app-shell">
      <section className="toolbar">
        <div>
          <h1>Housemouse Files</h1>
          <p>{status}</p>
        </div>
        <button type="button" onClick={refreshRoots}>
          Refresh
        </button>
      </section>

      <section className="panel">
        <label htmlFor="root-path">Managed root path</label>
        <div className="input-row">
          <input
            id="root-path"
            value={pathInput}
            onChange={(event) => setPathInput(event.target.value)}
            placeholder={demoRootPath}
          />
          <button type="button" onClick={() => setPathInput(demoRootPath)}>
            Demo
          </button>
          <button type="button" onClick={registerRoot} disabled={!pathInput.trim()}>
            Register
          </button>
        </div>
        <p className="path-text">
          Register is enabled after a path is entered. Tauri commands run inside the desktop app,
          not a normal browser tab.
        </p>
      </section>

      <section className="workspace-grid">
        <div className="panel">
          <label htmlFor="root-select">Registered roots</label>
          <select
            id="root-select"
            value={selectedRootId}
            onChange={(event) => setSelectedRootId(event.target.value)}
          >
            <option value="">No root selected</option>
            {roots.map((root) => (
              <option key={root.root_id} value={root.root_id}>
                {root.display_name}
              </option>
            ))}
          </select>
          {selectedRoot ? <p className="path-text">{selectedRoot.root}</p> : null}

          <div className="button-grid">
            <button type="button" onClick={analyzeSelectedRoot} disabled={!selectedRootId}>
              Analyze
            </button>
            <button type="button" onClick={proposeForSelectedRoot} disabled={!selectedRootId}>
              Propose
            </button>
            <button
              type="button"
              onClick={precheckSelectedRoot}
              disabled={!selectedRootId || !proposal}
            >
              Precheck
            </button>
            <button
              type="button"
              onClick={executeSelectedRoot}
              disabled={!selectedRootId || !proposal || approvedDecisions.length === 0}
            >
              Execute
            </button>
            <button type="button" onClick={undoSelectedRoot} disabled={!selectedRootId}>
              Undo
            </button>
          </div>
        </div>

        <div className="panel">
          <h2>Proposals</h2>
          <div className="proposal-list">
            {proposal?.proposals.map((item) => (
              <article key={item.proposal_id} className="proposal-row">
                <div>
                  <strong>{item.from}</strong>
                  <span>{item.to}</span>
                  <small>{item.status}</small>
                </div>
                <select
                  value={decisions[item.proposal_id] || "pending"}
                  onChange={(event) =>
                    setDecision(item, event.target.value as DecisionState[string])
                  }
                >
                  <option value="approved">Approve</option>
                  <option value="pending">Skip</option>
                  <option value="rejected">Reject</option>
                </select>
              </article>
            )) || <p>No proposal loaded.</p>}
          </div>
        </div>
      </section>

      <section className="panel">
        <h2>Output</h2>
        {error ? <p className="error-text">{error}</p> : null}
        <pre>{resultLines.join("\n") || "No output yet."}</pre>
      </section>
    </main>
  );
}

function formatProposal(item: Proposal) {
  return `${item.status} | ${item.from} -> ${item.to} | ${item.reason}`;
}

function errorMessage(caught: unknown) {
  return caught instanceof Error ? caught.message : String(caught);
}
