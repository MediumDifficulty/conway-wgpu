// https://github.com/bevyengine/bevy/blob/main/assets/shaders/game_of_life.wgsl

// The shader reads the previous frame's state from the `input` texture, and writes the new state of
// each pixel to the `output` texture. The textures are flipped each step to progress the
// simulation.
// Two textures are needed for the game of life as each pixel of step N depends on the state of its
// neighbors at step N-1.

@group(0) @binding(0) var input: texture_storage_2d<r32uint, read>;

@group(0) @binding(1) var output: texture_storage_2d<r32uint, write>;

fn is_alive(location: vec2<i32>, offset_x: i32, offset_y: i32) -> i32 {
    let value: vec4<u32> = textureLoad(input, location + vec2<i32>(offset_x, offset_y));
    return i32(value.r);
}

fn count_alive(location: vec2<i32>) -> i32 {
    return is_alive(location, -1, -1) +
           is_alive(location, -1,  0) +
           is_alive(location, -1,  1) +
           is_alive(location,  0, -1) +
           is_alive(location,  0,  1) +
           is_alive(location,  1, -1) +
           is_alive(location,  1,  0) +
           is_alive(location,  1,  1);
}

@compute @workgroup_size(8, 8, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let location = vec2<i32>(i32(invocation_id.x), i32(invocation_id.y));

    let n_alive = count_alive(location);

    var alive: bool;
    let currently_alive = is_alive(location, 0, 0);


    if (currently_alive == 1) {
        // Survival: live cell needs 2 or 3 neighbors to survive
        alive = n_alive == 2 || n_alive == 3;
    } else {
        // Birth: dead cell needs exactly 3 neighbors to become alive
        alive = n_alive == 3;
    }

    // alive = n_alive > i32(2);
    // if (n_alive <= 1) {
    //     alive = true;
    // }
    let data = vec4<u32>(u32(alive), 0, 0, 0);

    textureStore(output, location, data);
}
