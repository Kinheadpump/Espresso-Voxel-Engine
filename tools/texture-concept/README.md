# texture-concept

Phase 1 art tooling for `docs/TEXTURES.md`. Python 3 with numpy and Pillow; deterministic output.

```sh
python3 texgen.py OUT_DIR                 # 16 px and 32 px PNGs for every block face
python3 swatches.py ../../docs/textures/swatches.png
python3 mockup.py ../../docs/textures 640 360   # CPU ray-cast mock-ups (~4 min)
```

The mock-ups copy `terrain.wgsl`'s shading so only the surface varies. They are illustrations, not engine renders. Phase 2 ports `texgen.py` to a Rust tool.
