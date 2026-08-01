# Colosseum ticket lifecycle design

## Subject and job

Sanctum is the operator console for work tickets. Colosseum is the durable executioner. The ticket detail view has one job: let an operator understand what is happening, compare the proposed result with its starting point, understand the executioner's rationale, and approve or reject the final result.

## Lifecycle contract

```text
backlog -> grooming -> ready -> in-progress -> review -> human-review -> approved -> done
              |                      ^            |             |
              +-- needs input -------+-- failed --+-- rejected -+
```

- Backlog is human-authored and is never claimed by Colosseum.
- Grooming is claimed once. Colosseum adds a summary, assumptions, acceptance criteria, and explicit questions. It advances only when no clarification is required.
- Ready is claimed for work. Development work requires a repository and produces a task-owned worktree, branch, commit, pushed remote branch, Savant merge-request record, validation evidence, and a base-to-head diff. Research work produces a durable result comment and execution evidence without inventing a Git repository.
- Review is claimed separately with reviewer abilities. A failed review returns the ticket to Ready with findings. A passed review moves to Human review unless autopilot is enabled.
- Human approval queues a development merge or directly completes reviewed research. Rejection returns the ticket to Ready with the operator's reason.
- Autopilot performs the same merge/completion action immediately after a passing machine review. It skips only the human decision, not validation or machine review.

## Evidence contract

Every phase records a durable run entry in the task's Colosseum metadata and a readable task comment. Entries include phase, decision, rationale or summary, timestamps, provider/persona, validation outcome, worktree, branch, start/base commit, result commit, remote, merge-request ID, and log path when applicable.

The Server owns transitions and approval validation. Colosseum never infers approval from a free-form comment. Sanctum uses the same Server endpoints, so board state and worker state cannot diverge.

## Sanctum design plan

### Tokens

- Void navy `#080b12`: app background.
- Operations navy `#0d1220`: ticket surfaces.
- Instrument blue `#0f1929`: evidence rows.
- Signal cyan `#00e5ff`: active phase and primary action.
- Review magenta `#ff00aa`: review gates and human decisions.
- Verified green `#00ff88`: passing evidence and completion.
- Hazard red `#ff2244`: failures, rejection, and blocked work.

Typography remains the established Sanctum system: Orbitron for restrained phase labels, Rajdhani for readable controls, and Share Tech Mono for identifiers, commands, commits, and timestamps.

### Layout

```text
Left rail | Ticket queues                                | Right rail
          | backlog grooming ready work review human     |
          |----------------------------------------------|
          | ticket detail drawer, rail-to-rail           |
          | phase ledger                                 |
          | Details | Activity | Changes | Discussion    |
          | evidence + rationale      approve / reject   |
```

The signature element is the phase ledger: a compact, ordered strip whose segments expose the latest evidence and make stalled or failed transitions visually obvious. Motion is limited to a single live pulse on the active phase and respects reduced-motion preferences.

## Alternatives considered

1. A separate Colosseum dashboard would improve worker-centric monitoring, but split approval and ticket context across apps. Rejected because Sanctum is already the ticket source of truth.
2. Comments-only observability would be cheap, but cannot enforce approvals or provide structured phase comparison. Rejected as insufficient.
3. A structured execution ledger embedded in the ticket keeps the operator workflow in one place and supports both human and autopilot gates. Selected.

## Completion evidence

- Server tests prove valid transitions, original claimed phase preservation, ready filtering, run metadata, and approval/rejection rules.
- Rust tests prove phase dispatch, decision parsing, review outcomes, and merge safety.
- Sanctum tests prove all queues, evidence rendering, and approval controls.
- A live development ticket and a live research ticket complete the lifecycle; at least one uses human approval and one uses autopilot.
