# Contribution Rules
- Increment the version counter in VERSION.md for every task executed.
- Versioning rule: always increment the last position from 1 to 99; after reaching 99, increment the middle position by 1 and reset the last position to 0 (e.g., 0.0.99 -> 0.1.0).
- All functions and methods must have English comments explaining them.
- Update README.md at the end of tasks whenever documentation needs to be updated.
- Every implementation or fix must be accompanied by corresponding test(s) executed before the commit.
- Security expectations: validate grants/permissions, enforce minimum Proof-of-Work requirements where applicable, and avoid weakening authentication or authorization controls.
- If APIs change, documentation updates are mandatory (README.md and any relevant docs under /docs).
