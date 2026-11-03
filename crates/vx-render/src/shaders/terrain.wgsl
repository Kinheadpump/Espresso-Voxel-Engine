struct Camera {
    view_proj: mat4x4<f32>,
    anchor: vec4<i32>,
    eye: vec4<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<storage, read> quads: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> chunk_origins: array<vec4<i32>>;
@group(0) @binding(3) var<storage, read> block_faces: array<u32>;
@group(0) @binding(4) var block_textures: texture_2d_array<f32>;
@group(0) @binding(5) var block_sampler: sampler;

override TEXTURED: bool = true;
const TEXTURE_SIZE = 16.0;
const FACE_LAYER_MASK = 0xFFFFu;
const FACE_ROTATE_BIT = 16u;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) relative_position: vec3<f32>,
    @location(1) @interpolate(flat) block: u32,
    @location(2) @interpolate(flat) direction: u32,
    @location(3) @interpolate(flat) face: u32,
    @location(4) texture_position: vec3<f32>,
}

const CORNERS_FRONT = array<vec2<u32>, 6>(
    vec2(0u, 0u), vec2(1u, 0u), vec2(1u, 1u), vec2(0u, 0u), vec2(1u, 1u), vec2(0u, 1u),
);
const CORNERS_FLIPPED = array<vec2<u32>, 6>(
    vec2(0u, 0u), vec2(1u, 1u), vec2(1u, 0u), vec2(0u, 0u), vec2(0u, 1u), vec2(1u, 1u),
);

// Bit layout mirrors vx_mesh::PackedQuad: lo = x6 y6 z6 (w-1)6 (h-1)6 dir[0..2]; hi = dir[2] block16.
fn depth_u_v_to_xyz(axis: u32, d: u32, u: u32, v: u32) -> vec3<u32> {
    switch axis {
        case 0u: { return vec3(d, u, v); }
        case 1u: { return vec3(u, d, v); }
        default: { return vec3(u, v, d); }
    }
}

// block_faces word mirrors vx_core::BlockFaces::packed: top | side << 10 | bottom << 20 | rotate << 30.
// Output: layer | rotate << 16, rotate only for top and bottom faces.
fn face_texture(block: u32, direction: u32) -> u32 {
    let word = block_faces[select(0u, block, block < arrayLength(&block_faces))];
    let vertical = (direction >> 1u) == 1u;
    let face_class = select(1u, select(2u, 0u, direction == 2u), vertical);
    let layer = (word >> (10u * face_class)) & 0x3FFu;
    let rotate = select(0u, (word >> 30u) & 1u, vertical);
    return layer | (rotate << FACE_ROTATE_BIT);
}

// Texture coordinates repeat every block and the rotation hash only needs per-cell variety, so
// they use chunk origins wrapped to 4096 blocks: exact in f32 at any distance from the origin.
const TEXTURE_WRAP_MASK = vec3(4095);

struct QuadVertex {
    relative: vec3<f32>,
    texture_position: vec3<f32>,
    block: u32,
    direction: u32,
}

fn quad_vertex(vertex_index: u32, chunk_slot: u32) -> QuadVertex {
    let quad = quads[vertex_index / 6u];
    let lo = quad.x;
    let hi = quad.y;
    let origin = vec3(lo & 63u, (lo >> 6u) & 63u, (lo >> 12u) & 63u);
    let size = vec2(((lo >> 18u) & 63u) + 1u, ((lo >> 24u) & 63u) + 1u);
    let direction = (lo >> 30u) | ((hi & 1u) << 2u);
    let block = (hi >> 1u) & 0xFFFFu;

    let axis = direction >> 1u;
    let positive = (direction & 1u) == 0u;
    // Quad winding: u x v points along +axis for X and Z but along -axis for Y.
    let flip = positive == (axis == 1u);
    var corner = CORNERS_FRONT[vertex_index % 6u];
    if flip {
        corner = CORNERS_FLIPPED[vertex_index % 6u];
    }

    var depth_u_v: vec3<u32>;
    switch axis {
        case 0u: { depth_u_v = vec3(origin.x, origin.y, origin.z); }
        case 1u: { depth_u_v = vec3(origin.y, origin.x, origin.z); }
        default: { depth_u_v = vec3(origin.z, origin.x, origin.y); }
    }
    let local = depth_u_v_to_xyz(
        axis,
        depth_u_v.x + select(0u, 1u, positive),
        depth_u_v.y + corner.x * size.x,
        depth_u_v.z + corner.y * size.y,
    );
    let chunk_origin = chunk_origins[chunk_slot].xyz;
    return QuadVertex(
        vec3<f32>(chunk_origin - camera.anchor.xyz) + vec3<f32>(local),
        vec3<f32>(chunk_origin & TEXTURE_WRAP_MASK) + vec3<f32>(local),
        block,
        direction,
    );
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) chunk_slot: u32) -> VertexOut {
    let vertex = quad_vertex(vertex_index, chunk_slot);
    var out: VertexOut;
    out.clip_position = camera.view_proj * vec4(vertex.relative, 1.0);
    out.relative_position = vertex.relative;
    out.texture_position = vertex.texture_position;
    out.block = vertex.block;
    out.direction = vertex.direction;
    out.face = 0u;
    if TEXTURED {
        out.face = face_texture(vertex.block, vertex.direction);
    }
    return out;
}

struct ChunkIdOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) slot_plus_one: u32,
}

@vertex
fn vs_chunk_id(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) chunk_slot: u32) -> ChunkIdOut {
    let vertex = quad_vertex(vertex_index, chunk_slot);
    return ChunkIdOut(camera.view_proj * vec4(vertex.relative, 1.0), chunk_slot + 1u);
}

@fragment
fn fs_chunk_id(in: ChunkIdOut) -> @location(0) u32 {
    return in.slot_plus_one;
}

const BLOCK_COLORS = array<vec3<f32>, 6>(
    vec3(1.0, 0.0, 1.0),
    vec3(0.50, 0.50, 0.52),
    vec3(0.47, 0.33, 0.22),
    vec3(0.33, 0.62, 0.24),
    vec3(0.86, 0.80, 0.55),
    vec3(0.95, 0.96, 0.98),
);
const DIRECTION_SHADE = array<f32, 6>(0.80, 0.80, 1.00, 0.50, 0.65, 0.65);
const SKY_COLOR = vec3(0.34, 0.54, 0.83);
const FOG_DENSITY = 0.0012;

// Side faces use v = -y so textures stay upright on merged walls.
fn face_uv(world: vec3<f32>, axis: u32) -> vec2<f32> {
    switch axis {
        case 0u: { return vec2(world.z, -world.y); }
        case 1u: { return world.xz; }
        default: { return vec2(world.x, -world.y); }
    }
}

fn cell_hash(cell: vec2<i32>) -> u32 {
    var h = (bitcast<u32>(cell.x) * 0x8da6b343u) ^ (bitcast<u32>(cell.y) * 0xd8163841u);
    h = (h ^ (h >> 15u)) * 0x2c1b3c6du;
    return h ^ (h >> 12u);
}

fn rotate_quarter_turns(p: vec2<f32>, turns: u32) -> vec2<f32> {
    switch turns {
        case 1u: { return vec2(-p.y, p.x); }
        case 2u: { return -p; }
        case 3u: { return vec2(p.y, -p.x); }
        default: { return p; }
    }
}

// Texel-snapped linear filtering: inside a texel the UV is pinned to its centre, and the
// blend to the neighbour spans one screen pixel (|t - c| / footprint). Mip selection uses
// the unsnapped derivatives, so distant surfaces still filter smoothly.
fn textured_albedo(world: vec3<f32>, direction: u32, face: u32) -> vec3<f32> {
    let uv = face_uv(world, direction >> 1u);
    var ddx = dpdx(uv);
    var ddy = dpdy(uv);
    var sample_uv = uv;
    if ((face >> FACE_ROTATE_BIT) & 1u) == 1u {
        let cell = floor(uv);
        let turns = cell_hash(vec2<i32>(cell)) & 3u;
        sample_uv = cell + 0.5 + rotate_quarter_turns(uv - cell - 0.5, turns);
        ddx = rotate_quarter_turns(ddx, turns);
        ddy = rotate_quarter_turns(ddy, turns);
    }
    let texel = sample_uv * TEXTURE_SIZE;
    let footprint = max((abs(ddx) + abs(ddy)) * TEXTURE_SIZE, vec2(1e-6));
    let edge = floor(texel + 0.5);
    let snapped = edge + clamp((texel - edge) / footprint, vec2(-0.5), vec2(0.5));
    let layer = face & FACE_LAYER_MASK;
    return textureSampleGrad(block_textures, block_sampler, snapped / TEXTURE_SIZE, layer, ddx, ddy).rgb;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    var albedo = pow(BLOCK_COLORS[min(in.block, 5u)], vec3(2.2));
    if TEXTURED {
        albedo = textured_albedo(in.texture_position, in.direction, in.face);
    }
    let base = albedo * DIRECTION_SHADE[in.direction];
    let distance = length(in.relative_position - camera.eye.xyz);
    let fog = 1.0 - exp(-distance * FOG_DENSITY);
    return vec4(mix(base, SKY_COLOR, fog), 1.0);
}
