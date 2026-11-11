# Open points

A question that the documents do not answer becomes an open point (run rule 8). The task takes a safe default, records it in its task report, and continues. The orchestrator copies it here.

A `TODO` in the code names the id of its open point, for example `TODO(OP-003)`.

## Format

Each open point is one section:

```markdown
### OP-NNN: <the question, in one line>

- Raised by: <task id>
- Status: open | decided | closed
- Default: <the safe default that the code uses now>
- Where: <files or modules that hold the default>
- How to change: <what to edit when the decision changes>
- Decision: <empty while open; the decision and a link to its ADR>
```

## List

None yet.
