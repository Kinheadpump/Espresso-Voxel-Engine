# Documentation guideline

How every Markdown file in this repo is written. It applies to people and agents alike. When a doc breaks a rule here, fix the doc in the same PR that touches it.

## 1. Where each kind of fact lives

One fact has one home. Other docs link to it instead of copying it, so a number or a decision never exists in two versions.

| Kind of content | Home | Written how |
|---|---|---|
| Repo map, commands, invariants | `AGENTS.md` | Kept current; short |
| Claude-specific rules, engineering manifest | `CLAUDE.md` | Kept current; short |
| How the engine works today | `docs/ARCHITECTURE.md` | Describes the code at `main`, not plans |
| Design choices with alternatives | `docs/DECISIONS.md` | Append-only ADRs (section 5) |
| What we do next, in order | `docs/ROADMAP.md` | Numbered items, each with its verification |
| Sprints, their exit gates, long-term targets | `docs/SPRINTS.md` | One section per sprint; targets labelled **target** |
| Ideas not yet scheduled, ranked | `docs/BACKLOG.md` | `B-nn` items (section 6) |
| How to measure | `docs/BENCHMARKING.md` | Procedure and rules |
| Measured results | `docs/PERFORMANCE.md` | Tables with date, machine, command, commit |
| Raw benchmark output | `benchmarks/baselines/<machine-label>/` | JSON from `vx bench` plus a short README |
| What Decaf and Lemon taught us | `docs/HISTORY.md` | Summary; details in the audits |
| Research that took real effort | `docs/research/<topic>.md` or `docs/research/<survey>/` | Dated snapshot (section 7) |
| One feature's concept or plan | `docs/<FEATURE>.md` (e.g. `docs/TEXTURES.md`) | Plan plus its measurement plan; the decision itself goes into an ADR |

A measured number is written once, in `docs/PERFORMANCE.md` (or the baseline README). Other docs may restate it only with its label and a link, as in "~35 µs per surface chunk (measured, `docs/PERFORMANCE.md`)".

## 2. Structure of a doc

- **Title** (`# …`) says what the doc is, not what it argues.
- **First paragraph**: what the doc is for and who reads it, in one to three sentences. If the doc can go stale, state its date or the commit it describes.
- **Headings** are nouns or short statements (`## Storage`, `## Why it stalled`), at most three levels deep.
- **Order**: the answer or the current state first, the reasons and history after it. A reader who stops after the first screen should still have the right picture.
- **Tables** for anything compared across more than two items. Lists for steps and for unordered facts. Prose for reasoning.
- **Length**: split a doc that passes about 400 lines. Research surveys split by topic with a `README.md` index.
- **Links**: repo paths in backticks (`docs/BACKLOG.md`); relative Markdown links only inside a folder of related files. External sources as full URLs.

## 3. Tone

- English, plain and direct. Full sentences, short ones. No preamble, no closing summary, no marketing ("blazing", "massive").
- State findings as findings: "Data movement costs more than meshing" rather than "It seems that data movement might be expensive".
- Uncertainty is expressed through labels (section 4), not through hedging words.
- Name things the way the code names them (`ChunkWindow`, `gather_padded`), and define an abbreviation on first use in each doc.
- Units always: `ms`, `µs`, `MiB` (binary) and `MB` only when a source says MB. Ranges with an en dash (`17–24 µs`). Multipliers as `1.8×`.
- Dates as ISO `YYYY-MM-DD`. Never "today" or "last week".

## 4. Labelling numbers and claims

Every number that describes performance, size or cost carries exactly one of these labels:

| Label | Meaning | Must also give |
|---|---|---|
| **measured** | Produced in this repo by `vx bench`, `cargo bench-cpu` or `tools/legacy-compare`, release build | Machine (or baseline label), and where the full result is recorded |
| **quoted** | Copied from a source or an older doc, not re-run here | The source (URL, file, or commit) |
| **computed** | Arithmetic from other numbers | The inputs, or enough of them to redo it |
| **estimate** | A guess that a measurement will replace | The measurement that will replace it |
| **target** | A budget we want to hold | Nothing; never mix with results |

Rules:

- Write the label in bold or in parentheses right after the number: "58.6 MiB (measured)". In a table where every cell has the same provenance, put the label in the heading or caption once.
- Measured results and estimates never share a table column or a sentence without their labels. Keep "Now (measured)" and "Expected (estimate)" in separate paragraphs or columns.
- Software-adapter (lavapipe, WARP) GPU and frame times are labelled measured but always carry "software adapter, not GPU performance" next to them.
- A speed-up is only stated when both numbers come from the same machine and session and `vx bench-compare` puts the change outside the noise band. Otherwise say "within noise".
- Never round a measured number into a better-sounding one. Give ranges for repeat spreads.
- When a number is superseded, keep it, mark it **Superseded** with the date and a pointer to the new one. Do not delete it.

Claims about external sources (not numbers) carry a verification tag in research docs:

| Tag | Meaning |
|---|---|
| **[S]** | Seen this session: read on a fetched page or in cloned source. URL or repo given. |
| **[U]** | Unverified: background knowledge or own reasoning. |

## 5. ADR format (`docs/DECISIONS.md`)

```
## ADR-nnnn Short decision title (Status, YYYY-MM-DD)

Context in one or two sentences: what forced the choice.
Decision: what we do.
Alternatives: what else was considered, and why not.
Decided by: the measurement that settles it (required while Proposed).
```

- Status is one of **Proposed**, **Accepted**, **Rejected**, **Superseded by ADR-nnnn**.
- Append-only. A Proposed ADR may be edited until it is accepted. An Accepted ADR is never rewritten: only its status line changes when a later ADR supersedes it.
- Numbers in an ADR follow section 4.
- An ADR is needed for any change to the tech stack, the chunk size, the quad format, the threading model, or any choice the backlog lists alternatives for.

## 6. Backlog item format (`docs/BACKLOG.md`)

Each item has an id `B-nn` (stable, never reused), a row in the ranked table, and one section:

```
## B-nn Title

Origin: P-xx / R-xx / own finding, with the research doc that holds the sources.
Now (measured): what the code does today and what it costs, if known.
Change: what we would build.
Expected (estimate or quoted): the benefit, labelled.
Effort: S (under a day), M (a few days), L (rewrites a subsystem).
Decided by: the benchmark scenario and metric, and the threshold that keeps or reverts it.
```

An item leaves the backlog by getting a roadmap slot (and an ADR if it is a design choice), or by moving to "Not adopted" with the reason. Its id stays.

## 7. Research docs (`docs/research/`)

- Research docs are dated snapshots. The first lines give the date, the question, and the tag legend (section 4).
- Each one ends with **Gaps**: what could not be fetched or verified.
- Every candidate it recommends gets a backlog entry; the research doc keeps the evidence, the backlog keeps the ranking.
- After the date, research docs are not rewritten to match new findings. Add a dated note at the top instead ("2026-12-01: superseded on X by …").

## 8. Commit and PR text

Follows `CLAUDE.md` (Conventional Commits, Stats section). The labels in section 4 apply there too: Stats only contain measured values with the machine named, or `n/a`.

## 9. Checklist before merging a docs change

- Every number has a label and a source.
- No measured value and estimate are mixed without labels.
- No fact is duplicated where a link would do.
- Status lines and dates are current (roadmap ticks, ADR status, "as of" dates).
- Links and paths resolve.
