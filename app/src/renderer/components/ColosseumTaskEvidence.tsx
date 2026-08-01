import { History } from 'lucide-react';
import type { ColosseumRun, Task } from '../data';
import { COLOSSEUM_PHASES } from '../lib/taskBoard';

export function ColosseumPhaseLedger({ state }: { state: Task['state'] }) {
  const currentIndex = COLOSSEUM_PHASES.indexOf(state);
  return (
    <div className="colosseum-phase-ledger" aria-label="Colosseum lifecycle">
      {COLOSSEUM_PHASES.map((phase, index) => {
        const phaseState = index < currentIndex ? 'complete' : index === currentIndex ? 'active' : 'pending';
        return (
          <div key={phase} className={`colosseum-phase colosseum-phase-${phaseState}`}>
            <span>{index + 1}</span>
            <strong>{phase.replace('-', ' ')}</strong>
          </div>
        );
      })}
    </div>
  );
}

function RunCard({ run }: { run: ColosseumRun }) {
  return (
    <article className="colosseum-run-card">
      <header>
        <span className={`colosseum-run-phase colosseum-run-phase-${run.phase}`}>{run.phase}</span>
        <strong>{run.status}</strong>
        <time>{run.created_at ? new Date(run.created_at).toLocaleString() : 'time unavailable'}</time>
      </header>
      {run.summary && <p>{run.summary}</p>}
      {run.rationale && (
        <div className="colosseum-run-rationale"><strong>Why</strong><span>{run.rationale}</span></div>
      )}
      {(run.questions ?? []).length > 0 && (
        <div className="colosseum-run-rationale">
          <strong>Questions</strong><span>{run.questions?.join('\n')}</span>
        </div>
      )}
      <footer>
        {run.persona && <span>persona {run.persona.replace('persona.', '')}</span>}
        {run.provider && <span>provider {run.provider}</span>}
        {run.model && <span>model {run.model}</span>}
        {run.branch && <span>branch {run.branch}</span>}
        {run.commit && <span>commit {run.commit.slice(0, 10)}</span>}
        {run.mr_id && <span>MR {run.mr_id}</span>}
        {run.log_path && <span title={run.log_path}>log available</span>}
      </footer>
    </article>
  );
}

export function ColosseumRunLedger({ runs }: { runs: ColosseumRun[] }) {
  return (
    <div className="colosseum-run-ledger">
      <div className="colosseum-run-ledger-head">
        <div><span>Execution evidence</span><strong>{runs.length} recorded runs</strong></div>
        <small>Every grooming, work, review, and merge decision remains attached to this ticket.</small>
      </div>
      <div className="colosseum-run-list">
        {runs.length === 0 ? (
          <div className="colosseum-run-empty">
            <History size={24} /><strong>No Colosseum runs yet</strong>
            <span>Send this ticket to grooming to begin the execution record.</span>
          </div>
        ) : [...runs].reverse().map((run) => <RunCard key={run.run_id} run={run} />)}
      </div>
    </div>
  );
}
