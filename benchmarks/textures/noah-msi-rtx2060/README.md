# Flat vs textured shading: noah-msi-rtx2060

Measured on 2026-11-03 at commit `8a8bea163fc5` (PR #8 merged), release build, default bench settings (5 repeats, 120 warm-up + 600 measured frames, 1920×1080). Machine: MSI laptop on AC power with the High performance plan, Intel Core i7-9750H, NVIDIA GeForce RTX 2060, Vulkan, driver 616.92. Nothing else used the GPU during the runs.

Budget: the textured GPU terrain pass may be at most 10% slower than flat.

## GPU terrain pass (median of repeat medians)

| Scenario | Order | Flat | Textured | Change | Noise | Within 10% budget |
|---|---|---|---|---|---|---|
| default | flat first | 508.8 µs | 552.7 µs | +8.6% | 6.9% | yes |
| default | textured first (run 2) | 520.2 µs | 561.0 µs | +7.8% | 3.6% | yes |
| far | flat first | 1769.5 µs | 1816.8 µs | +2.7% | 2.0% | yes |

The difference is real (outside the noise band in every run) and is below the 10% budget. The `default` cost is the closer one to the budget.

## Other metrics

- Generation, meshing and upload: within noise. The shading change does not touch them.
- Frame CPU encode and frame wall in the first `default` run: +28.8% and +28.0%. The second run, in reverse order, gave −0.3% and +3.2%, both within noise. The first flat run had a low CPU encode time (341.9 µs against 438.1 µs in run 2), so the +28% is CPU-side run-to-run noise on this laptop, not a texture cost.
- `far`: CPU encode +2.8%, frame wall +2.6%, just outside the 2% noise floor.

## Files

- `<scenario>-<shading>-8a8bea1.json`: run 1 (flat, then textured).
- `default-<shading>-8a8bea1-run2.json`: run 2 (textured, then flat).
- `*-compare*.txt`: `vx bench-compare` output.
