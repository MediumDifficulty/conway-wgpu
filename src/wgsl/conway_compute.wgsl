#import common

@group(0) @binding(0) var input: texture_storage_2d<r32uint, read>;

@group(0) @binding(1) var output: texture_storage_2d<r32uint, write>;

// Most significant -> LEFT
// Least significant -> RIGHT

fn count_right(current: u32, above: u32, below: u32, pixel_loc: vec2i) -> u32 {
    return ((textureLoad(input, pixel_loc + vec2i(1, -1)).r >> 31u) & 1u) + // NE
        ((textureLoad(input, pixel_loc + vec2i(1, 1)).r >> 31u) & 1u) + // SE
        ((textureLoad(input, pixel_loc + vec2i(1, 0)).r >> 31u) & 1u) + // E
        countOneBits(extractBits(above, 0u, 2u)) +
        countOneBits(extractBits(below, 0u, 2u)) +
        ((current >> 1u) & 1u);
}

fn count_left(current: u32, above: u32, below: u32, pixel_loc: vec2i) -> u32 {
    return (textureLoad(input, pixel_loc + vec2i(-1, -1)).r & 1u) + // NW
        (textureLoad(input, pixel_loc + vec2i(-1, 1)).r & 1u) + // SW
        (textureLoad(input, pixel_loc + vec2i(-1, 0)).r & 1u) + // W
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

@compute @workgroup_size(8, 8, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let location = vec2i(invocation_id.xy);

    let above = textureLoad(input, location + vec2i(0, -1)).r;
    let below = textureLoad(input, location + vec2i(0, 1)).r;
    let current = textureLoad(input, location).r;

    var out = 0u;

    // left to right
    for (var i = 1u; i < common::BITS_PER_PIXEL - 1; i++) {
        let neighbors = count_middle(current, above, below, i);
        let alive = ((current >> i) & 1u) == 1u;
        out |= u32(map(neighbors, alive)) << i;
    }

    out |= u32(map(count_left(current, above, below, location), ((current >> 31u) & 1u) == 1u)) << 31u;
    out |= u32(map(count_right(current, above, below, location), (current & 1u) == 1u));

    let data = vec4<u32>(out, 0u, 0u, 0u);
    textureStore(output, location, data);
}
