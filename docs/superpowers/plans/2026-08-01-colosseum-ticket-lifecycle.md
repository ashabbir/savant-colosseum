# Colosseum ticket lifecycle implementation plan

1. Add failing Server tests for queue selection, claimed phase preservation, execution metadata, and approval transitions.
2. Implement the shared Server lifecycle contract and focused API endpoints.
3. Add failing Rust tests for phase routing and structured agent decisions.
4. Implement grooming, work, review, human approval merge, research completion, and autopilot in Colosseum.
5. Add failing Sanctum tests for lifecycle mapping and human decision controls.
6. Implement the phase ledger, activity evidence, diff context, work type/autopilot configuration, and approve/reject UI.
7. Analyze every changed production file with Savant Context, clean findings, and rerun analysis.
8. Run Server, Rust, and Sanctum tests/builds; exercise live development and research tickets; commit and push coherent changes.
