import { useEffect, useMemo, useState } from "react";

import {
  analyzeRoot,
  DecisionEntry,
  executeFileChanges,
  ExecuteReport,
  listManagedRoots,
  ManagedRoot,
  precheckFileChanges,
  Proposal,
  ProposalReport,
  proposeFileChanges,
  registerManagedRoot,
  selectManagedRootDirectory,
  undoLastFileOperation,
  UndoReport
} from "./fileEngineApi";

type DecisionState = Record<string, "approved" | "rejected" | "pending">;
type RejectionReasons = Record<string, string>;

type HistoryEntry = {
  id: string;
  kind: "execute" | "undo";
  rootId: string;
  rootName: string;
  createdAt: string;
  summary: string;
  lines: string[];
};

const demoRootPath =
  "C:\\Users\\user\\2026-project\\kaist_madcamp\\week2\\test-fixtures\\file-trees\\ui-demo";
const historyKey = "housemouse.fileEngine.history";

export function FileEnginePanel() {
  const [pathInput, setPathInput] = useState("");
  const [roots, setRoots] = useState<ManagedRoot[]>([]);
  const [selectedRootId, setSelectedRootId] = useState("");
  const [proposal, setProposal] = useState<ProposalReport | null>(null);
  const [decisions, setDecisions] = useState<DecisionState>({});
  const [rejectionReasons, setRejectionReasons] = useState<RejectionReasons>({});
  const [history, setHistory] = useState<HistoryEntry[]>(() => loadHistory());
  const [status, setStatus] = useState("Ready");
  const [error, setError] = useState<string | null>(null);
  const [resultLines, setResultLines] = useState<string[]>([]);

  const selectedRoot = roots.find((root) => root.root_id === selectedRootId);
  const readyProposalCount = proposal?.proposals.filter((item) => item.status === "ready").length ?? 0;
  const commandDecisions = useMemo(
    () => buildDecisionEntries(decisions, rejectionReasons),
    [decisions, rejectionReasons]
  );
  const approvedCount = commandDecisions.filter((decision) => decision.decision === "approved").length;

  useEffect(() => {
    void refreshRoots();
  }, []);

  useEffect(() => {
    localStorage.setItem(historyKey, JSON.stringify(history));
  }, [history]);

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

  async function browseForRoot() {
    setError(null);
    try {
      const selected = await selectManagedRootDirectory();
      if (selected) {
        setPathInput(selected);
      }
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
      selectRoot(managed.root_id);
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
      setRejectionReasons({});
      setResultLines(report.proposals.map(formatProposal));
      setStatus(`Prepared ${report.proposals.length} proposals`);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Proposal failed");
    }
  }

  async function precheckSelectedRoot() {
    if (!selectedRootId || !proposal) return;

    const decisionsToApply = validatedDecisionEntries();
    if (!decisionsToApply) return;

    setError(null);
    setStatus("Prechecking");
    try {
      const report = await precheckFileChanges(selectedRootId, proposal, decisionsToApply);
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

    const decisionsToApply = validatedDecisionEntries();
    if (!decisionsToApply) return;

    setError(null);
    setStatus("Prechecking before execute");
    try {
      const precheck = await precheckFileChanges(selectedRootId, proposal, decisionsToApply);
      const blocked = precheck.checks.filter((check) => check.status !== "ready");
      if (blocked.length > 0) {
        setResultLines(
          blocked.map((check) =>
            [check.status, `${check.from} -> ${check.to}`, check.reason].filter(Boolean).join(" | ")
          )
        );
        setStatus("Execute blocked by precheck");
        return;
      }

      const confirmed = window.confirm(
        `Move ${precheck.checks.length} approved files for ${selectedRoot?.display_name || "this root"}?`
      );
      if (!confirmed) {
        setStatus("Execute cancelled");
        return;
      }

      setStatus("Executing");
      const report = await executeFileChanges(selectedRootId, proposal, decisionsToApply);
      const lines = formatExecuteLines(report);
      setResultLines(lines);
      setStatus(
        `Executed ${report.executed_count}, skipped ${report.skipped_count}, rejected ${report.rejected_count}`
      );
      recordHistory("execute", `Executed ${report.executed_count}, skipped ${report.skipped_count}`, lines);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Execute failed");
    }
  }

  async function undoSelectedRoot() {
    if (!selectedRootId) return;

    const confirmed = window.confirm(`Undo latest file operation for ${selectedRoot?.display_name || "this root"}?`);
    if (!confirmed) return;

    setError(null);
    setStatus("Undoing");
    try {
      const report = await undoLastFileOperation(selectedRootId);
      const lines = formatUndoLines(report);
      setResultLines(lines);
      setStatus(`Undone ${report.undone_count}, skipped ${report.skipped_count}`);
      recordHistory("undo", `Undone ${report.undone_count}, skipped ${report.skipped_count}`, lines);
    } catch (caught) {
      setError(errorMessage(caught));
      setStatus("Undo failed");
    }
  }

  function selectRoot(rootId: string) {
    setSelectedRootId(rootId);
    setProposal(null);
    setDecisions({});
    setRejectionReasons({});
    setResultLines([]);
  }

  function setDecision(item: Proposal, decision: DecisionState[string]) {
    setDecisions((current) => ({ ...current, [item.proposal_id]: decision }));
  }

  function setRejectionReason(item: Proposal, reason: string) {
    setRejectionReasons((current) => ({ ...current, [item.proposal_id]: reason }));
  }

  function validatedDecisionEntries() {
    const missingReason = proposal?.proposals.find(
      (item) => decisions[item.proposal_id] === "rejected" && !rejectionReasons[item.proposal_id]?.trim()
    );

    if (missingReason) {
      setError(`Rejected proposal needs a reason: ${missingReason.from}`);
      setStatus("Decision invalid");
      return null;
    }

    if (commandDecisions.length === 0) {
      setError("Approve or reject at least one proposal before continuing.");
      setStatus("No decisions selected");
      return null;
    }

    return commandDecisions;
  }

  function recordHistory(kind: HistoryEntry["kind"], summary: string, lines: string[]) {
    const entry: HistoryEntry = {
      id: `${Date.now()}-${kind}`,
      kind,
      rootId: selectedRootId,
      rootName: selectedRoot?.display_name || selectedRootId,
      createdAt: new Date().toLocaleString(),
      summary,
      lines
    };

    setHistory((current) => [entry, ...current].slice(0, 20));
  }

  function clearHistory() {
    setHistory([]);
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
          <button type="button" onClick={browseForRoot}>
            Browse
          </button>
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
            onChange={(event) => selectRoot(event.target.value)}
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
              disabled={!selectedRootId || !proposal || approvedCount === 0}
            >
              Execute
            </button>
            <button type="button" onClick={undoSelectedRoot} disabled={!selectedRootId}>
              Undo
            </button>
          </div>
          {proposal ? (
            <p className="path-text">
              {readyProposalCount} ready, {approvedCount} approved
            </p>
          ) : null}
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
                  {decisions[item.proposal_id] === "rejected" ? (
                    <input
                      value={rejectionReasons[item.proposal_id] || ""}
                      onChange={(event) => setRejectionReason(item, event.target.value)}
                      placeholder="Reason for rejection"
                    />
                  ) : null}
                </div>
                <select
                  value={decisions[item.proposal_id] || "pending"}
                  onChange={(event) =>
                    setDecision(item, event.target.value as DecisionState[string])
                  }
                >
                  <option value="approved" disabled={item.status !== "ready"}>
                    Approve
                  </option>
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

      <section className="panel">
        <div className="section-header">
          <h2>History</h2>
          <button type="button" onClick={clearHistory} disabled={history.length === 0}>
            Clear
          </button>
        </div>
        <div className="history-list">
          {history.map((entry) => (
            <article key={entry.id} className="history-row">
              <strong>
                {entry.kind} | {entry.rootName}
              </strong>
              <span>{entry.summary}</span>
              <small>{entry.createdAt}</small>
            </article>
          ))}
          {history.length === 0 ? <p>No history yet.</p> : null}
        </div>
      </section>
    </main>
  );
}

function buildDecisionEntries(decisions: DecisionState, rejectionReasons: RejectionReasons) {
  return Object.entries(decisions)
    .filter(([, decision]) => decision !== "pending")
    .map(([proposal_id, decision]): DecisionEntry => {
      if (decision === "approved") {
        return { proposal_id, decision };
      }

      if (decision === "rejected") {
        return {
          proposal_id,
          decision,
          reason: rejectionReasons[proposal_id]?.trim()
        };
      }

      throw new Error(`Unsupported decision: ${decision}`);
    });
}

function formatProposal(item: Proposal) {
  return `${item.status} | ${item.from} -> ${item.to} | ${item.reason}`;
}

function formatExecuteLines(report: ExecuteReport) {
  return report.results.map((result) =>
    [result.status, `${result.from} -> ${result.to}`, result.reason].filter(Boolean).join(" | ")
  );
}

function formatUndoLines(report: UndoReport) {
  return report.results.map((result) =>
    [result.status, `${result.from} -> ${result.to}`, result.reason].filter(Boolean).join(" | ")
  );
}

function loadHistory() {
  try {
    const value = localStorage.getItem(historyKey);
    return value ? (JSON.parse(value) as HistoryEntry[]) : [];
  } catch {
    return [];
  }
}

function errorMessage(caught: unknown) {
  return caught instanceof Error ? caught.message : String(caught);
}
