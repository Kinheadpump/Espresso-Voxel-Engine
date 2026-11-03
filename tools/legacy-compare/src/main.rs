use legacy_compare::{
    adapt::{DecafInput, LemonInput, decaf_mesh, lemon_mesh},
    decaf::ChunkMeshSlices,
    scenes::{SCENE_NAMES, build},
    unit_faces,
};
use vx_mesh::{BinaryGreedyMesher, ChunkMesh, mesh_culled_unit_faces};

fn main() {
    println!("scene,unit_faces,quads_espresso,quads_decaf,quads_lemon,all_equal");
    let mut all_ok = true;
    for name in SCENE_NAMES {
        let padded = build(name);
        let mut reference_quads = Vec::new();
        mesh_culled_unit_faces(&padded, &mut reference_quads);
        let reference = unit_faces::from_engine_quads(&reference_quads);

        let mut mesh = ChunkMesh::default();
        BinaryGreedyMesher::default().mesh(&padded, &mut mesh);
        let espresso = unit_faces::from_engine_mesh(&mesh);

        let decaf_input = DecafInput::from_padded(&padded);
        let mut slices = ChunkMeshSlices::new();
        decaf_mesh::<true>(&decaf_input, &mut slices);
        let (decaf_faces, decaf_quads, decaf_transparent) = unit_faces::from_decaf(&slices);
        assert_eq!(decaf_transparent, 0);

        let (lemon_faces, lemon_quads) =
            unit_faces::from_lemon(&lemon_mesh(&LemonInput::from_padded(&padded)));

        let equal = espresso == reference && decaf_faces == reference && lemon_faces == reference;
        all_ok &= equal;
        println!(
            "{name},{},{},{decaf_quads},{lemon_quads},{equal}",
            reference.len(),
            mesh.quads.len()
        );
    }
    assert!(all_ok, "unit-face sets differ");
}
