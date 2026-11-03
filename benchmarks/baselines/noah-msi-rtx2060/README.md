# noah-msi-rtx2060

Hardware baseline measured on 2026-11-03 at commit `77dacdfae05a`, release build, default settings (5 repeats, 120 warm-up + 600 measured frames, 1920×1080).

- Machine: MSI laptop, Windows 11 Home, on AC power, power plan "High performance"
- CPU: Intel Core i7-9750H (6 cores, 12 threads)
- GPU: NVIDIA GeForce RTX 2060, Vulkan, discrete, driver 616.92 (the Intel UHD 630 iGPU was not used)

Files:

- `*-77dacdfae05a.json`: the reference run (run 2). Compare later changes against these.
- `*-77dacdfae05a-run1.json`: run 1, about 4 minutes earlier, kept for the stability check.
- `adapter.txt`: `vx --probe` output (identical for both runs).

Stability check (`vx bench-compare` run 1 → run 2):

- `default`: every metric within noise.
- `small`: upload flagged SLOWER (+47%, noise 44%); everything else within noise.
- `far`: world generation (−14%, noise 12%) and frame wall (−14%, noise 13%) flagged FASTER; everything else within noise.

The `small` and `far` flags are run-to-run variation on this laptop, not code changes. Treat upload and the `far` CPU timings on this machine as noisy.

## Occlusion scenarios (2026-11-03, commit `b2ee6a175ccc`, PR #9)

- `cave-b2ee6a175ccc.json`, `mountain-b2ee6a175ccc.json`, `fly-fast-b2ee6a175ccc.json`: first runs of the B-01 scenarios, default settings, same machine, AC power, "Höchstleistung" (High performance) plan, driver 616.92. Each report includes the culling ladder (`visibility`).
- The before/after check of PR #9 against `229487f` (`default` twice, `far` once) is summarised in `docs/PERFORMANCE.md`; its raw files were not committed.
