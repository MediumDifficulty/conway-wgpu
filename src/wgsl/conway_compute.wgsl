#import common

@group(0) @binding(0) var input: texture_storage_2d<rg32uint, read>;

@group(0) @binding(1) var output: texture_storage_2d<rg32uint, write>;


// fn count_right(current: u32, above: u32, below: u32, pixel_loc: vec2i) -> u32 {
//     return ((textureLoad(input, pixel_loc + vec2i(1, -1)).r >> 31u) & 1u) + // NE
//         ((textureLoad(input, pixel_loc + vec2i(1, 1)).r >> 31u) & 1u) + // SE
//         ((textureLoad(input, pixel_loc + vec2i(1, 0)).r >> 31u) & 1u) + // E
//         countOneBits(extractBits(above, 0u, 2u)) +
//         countOneBits(extractBits(below, 0u, 2u)) +
//         ((current >> 1u) & 1u);
// }

// fn count_left(current: u32, above: u32, below: u32, pixel_loc: vec2i) -> u32 {
//     return (textureLoad(input, pixel_loc + vec2i(-1, -1)).r & 1u) + // NW
//         (textureLoad(input, pixel_loc + vec2i(-1, 1)).r & 1u) + // SW
//         (textureLoad(input, pixel_loc + vec2i(-1, 0)).r & 1u) + // W
//         countOneBits(extractBits(above, 30u, 2u)) +
//         countOneBits(extractBits(below, 30u, 2u)) +
//         ((current >> 30u) & 1u);
// }

fn count_west(current: u32, n: u32, s: u32, nw: u32, w: u32, sw: u32) -> u32 {
    return (nw & 1u) +
        (w & 1u) +
        (sw & 1u) +
        countOneBits(n & 7u) +
        countOneBits(s & 7u) +
        ((current >> 1u) & 1u);
}

fn count_east(current: u32, n: u32, s: u32, ne: u32, e: u32, se: u32) -> u32 {
    return ((ne >> 7u) & 1u) +
        ((e >> 7u) & 1u) +
        ((se >> 7u) & 1u) +
        countOneBits(extractBits(n, 6u, 2u)) +
        countOneBits(extractBits(s, 6u, 2u)) +
        ((current >> 6u) & 1u);
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

fn chunk_row(chunk: u32, row: u32) -> u32 {
    return extractBits(chunk, row * 8u, 8u) >> (row * 8u);
}

fn process_chunk(
    chunk: u32, n: u32, ne: u32, e: u32, se: u32, s: u32, sw: u32, w: u32, nw: u32
) -> u32 {
    var out = 0u;

    // NW Corner
    out |= u32(map(count_west(
        chunk_row(chunk, 0u),
        chunk_row(n, 7u),
        chunk_row(chunk, 1u),
        chunk_row(nw, 7u),
        chunk_row(w, 0u),
        chunk_row(w, 1u)
    ), (chunk & 1u) == 1u));

    // Top
    for (var i = 1u; i < 7u; i++) {
        let neighbors = count_middle(
            chunk_row(chunk, 0u),
            chunk_row(n, 7u),
            chunk_row(chunk, 1u),
            i
        );
        let alive = ((chunk >> i) & 1u) == 1u;
        out |= u32(map(neighbors, alive)) << i;
    }

    // NE Corner
    out |= u32(map(count_east(
        chunk_row(chunk, 0u),
        chunk_row(n, 7u),
        chunk_row(chunk, 1u),
        chunk_row(ne, 7u),
        chunk_row(e, 0u),
        chunk_row(e, 1u)
    ), ((chunk >> 7u) & 1u) == 1u)) << 7u;

    for (var i = 1u; i < 3u; i++) {
        // West
        out |= u32(map(count_west(
            chunk_row(chunk, i),
            chunk_row(chunk, i - 1u),
            chunk_row(chunk, i + 1u),
            chunk_row(w, i - 1u),
            chunk_row(w, i),
            chunk_row(w, i + 1u)
        ), ((chunk >> (8u * i)) & 1u) == 1u)) << (8u * i);
        // Middle
        for (var j = 1u; j < 7u; j++) {
            out |= u32(map(count_middle(
                chunk_row(chunk, i),
                chunk_row(chunk, i - 1u),
                chunk_row(chunk, i + 1u),
                j
            ), ((chunk >> (8u * i + j)) & 1u) == 1u)) << (8u * i + j);
        }
        // East
    }

    // SW Corner
    // Bottom
    // SE Corner

    return out;
}

@compute @workgroup_size(8, 8, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    // let location = vec2i(invocation_id.xy);

    // let above = textureLoad(input, location + vec2i(0, -1)).r;
    // let below = textureLoad(input, location + vec2i(0, 1)).r;
    // let current = textureLoad(input, location).r;

    // var out = 0u;

    // // left to right
    // for (var i = 1u; i < common::BITS_PER_PIXEL - 1; i++) {
    //     let neighbors = count_middle(current, above, below, i);
    //     let alive = ((current >> i) & 1u) == 1u;
    //     out |= u32(map(neighbors, alive)) << i;
    // }

    // out |= u32(map(count_left(current, above, below, location), ((current >> 31u) & 1u) == 1u)) << 31u;
    // out |= u32(map(count_right(current, above, below, location), (current & 1u) == 1u));

    // let data = vec4<u32>(out, 0u, 0u, 0u);
    // textureStore(output, location, data);
}
