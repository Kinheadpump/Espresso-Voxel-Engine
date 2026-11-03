# texture-concept

Engine block textures: `python3 blocks.py ../../assets/blocks [--preview sheet.png]` (revision 2, the PNGs the engine embeds).

Phase 1 art tooling for `docs/TEXTURES.md`. Python 3 with numpy and Pillow; deterministic output.

```sh
python3 texgen.py OUT_DIR                 # 16 px and 32 px PNGs for every block face
python3 swatches.py ../../docs/textures/swatches.png
python3 mockup.py ../../docs/textures 640 360   # CPU ray-cast mock-ups (~4 min)
```

The mock-ups copy `terrain.wgsl`'s shading so only the surface varies. They are illustrations, not engine renders.
