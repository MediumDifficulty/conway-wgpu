#import common

@group(0) @binding(0) var input: binding_array< texture_storage_2d<r32uint, read> >;
@group(0) @binding(1) var output: binding_array< texture_storage_2d<r32uint, write> >;

// Most significant -> LEFT
// Least significant -> RIGHT

fn get_pixel(pos: vec2i) -> u32 {
    if (pos.x < 0 || pos.y < 0) {
        return 0u;
    }

    let tile_pos = vec2u(pos / #TILE_SIZE);
    let texture_pos = vec2u(pos % #TILE_SIZE);

    return textureLoad(input[tile_pos.y * #GRID_WIDTH + tile_pos.x], texture_pos).r;
}

fn set_pixel(pos: vec2i, value: u32) {
    if (pos.x < 0 || pos.y < 0) {
        return;
    }

    let tile_pos = vec2u(pos / #TILE_SIZE);
    let texture_pos = vec2u(pos % #TILE_SIZE);

    textureStore(output[tile_pos.y * #GRID_WIDTH + tile_pos.x], texture_pos, vec4u(value, 0, 0, 0));
}

fn count_right(current: u32, above: u32, below: u32, pixel_loc: vec2i) -> u32 {
    return ((get_pixel(pixel_loc + vec2i(1, -1)) >> 31u) & 1u) + // NE
        ((get_pixel(pixel_loc + vec2i(1, 1)) >> 31u) & 1u) + // SE
        ((get_pixel(pixel_loc + vec2i(1, 0)) >> 31u) & 1u) + // E
        countOneBits(extractBits(above, 0u, 2u)) +
        countOneBits(extractBits(below, 0u, 2u)) +
        ((current >> 1u) & 1u);
}

fn count_left(current: u32, above: u32, below: u32, pixel_loc: vec2i) -> u32 {
    return (get_pixel(pixel_loc + vec2i(-1, -1)) & 1u) + // NW
        (get_pixel(pixel_loc + vec2i(-1, 1)) & 1u) + // SW
        (get_pixel(pixel_loc + vec2i(-1, 0)) & 1u) + // W
        countOneBits(extractBits(above, 30u, 2u)) +
        countOneBits(extractBits(below, 30u, 2u)) +
        ((current >> 30u) & 1u);
}

fn count_middle(current: u32, above: u32, below: u32, pos: u32) -> u32 {
    return countOneBits(extractBits(above, pos - 1u, 3u)) +
        countOneBits(extractBits(below, pos - 1u, 3u)) +
        countOneBits(extractBits(current, pos - 1u, 3u)) -
        ((current >> pos) & 1u);
}

fn map(neighbors: u32, alive: bool) -> bool {
    if (alive) {
        // Survival: live cell needs 2 or 3 neighbors to survive
        return neighbors == 2u || neighbors == 3u;
    } else {
        // Birth: dead cell needs exactly 3 neighbors to become alive
        return neighbors == 3u;
    }
}

// https://marc-b-reynolds.github.io/math/2016/03/29/weyl_hash.html
const W0 = 0x3504f333u;   // 3*2309*128413
const W1 = 0xf1bbcdcbu;   // 7*349*1660097
const M = 741103597u;    // 13*83*686843

fn hash(pos: vec2u) -> u32 {
    var x = pos.x;
    var y = pos.y;

    x *= W0;   // x' = Fx(x)
    y *= W1;   // y' = Fy(y)
    x ^= y;    // combine
    x *= M;    // MLCG constant
    return x;
}

@compute @workgroup_size(8, 8, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    // set_pixel(vec2i(invocation_id.xy), 1u);
    let lit = (hash(vec2u(invocation_id.x << 16u, invocation_id.y << 16u)) & 1u) == 0u;

    if lit {
        let data = hash(invocation_id.xy);
        set_pixel(vec2i(invocation_id.xy), data);
    }
}

@compute @workgroup_size(8, 8, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let location = vec2i(invocation_id.xy);

    let above = get_pixel(location + vec2i(0, -1));
    let below = get_pixel(location + vec2i(0, 1));
    let current = get_pixel(location);

    var out = 0u;

    // left to right
    for (var i = 1u; i < common::BITS_PER_PIXEL - 1; i++) {
        let neighbors = count_middle(current, above, below, i);
        let alive = ((current >> i) & 1u) == 1u;
        out |= u32(map(neighbors, alive)) << i;
    }

    out |= u32(map(count_left(current, above, below, location), ((current >> 31u) & 1u) == 1u)) << 31u;
    out |= u32(map(count_right(current, above, below, location), (current & 1u) == 1u));

    set_pixel(location, out);
}
