#import common

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

struct CameraUniform {
    screen_resolution: vec2<f32>,
    centre: vec2<f32>,
    zoom: f32
}


@group(0) @binding(0) var world: binding_array< texture_storage_2d<r32uint, read> >;
@group(1) @binding(0) var<uniform> camera: CameraUniform;

fn get_pixel(pos: vec2i) -> u32 {
    let tile_pos = vec2u(pos / #TILE_SIZE);
    let texture_pos = vec2u(pos % #TILE_SIZE);
    let tile_index = (tile_pos.y * #GRID_WIDTH) + tile_pos.x;
    if tile_index >= #GRID_LENGTH {
        return 0u;
    }
    return textureLoad(world[tile_index], texture_pos).r;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;
    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;
    let tc = vec2<f32>(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    result.position = vec4<f32>(
        tc.x * 2.0 - 1.0,
        1.0 - tc.y * 2.0,
        0.0, 1.0
    );
    result.tex_coords = tc;
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = (vertex.tex_coords * camera.screen_resolution - camera.screen_resolution / 2.) * camera.zoom + camera.centre;
    let pos = vec2i(world_pos);
    if pos.x < 0 || pos.y < 0 {
        return vec4f(0.);
    }
    // return vec4<f32>(fract(world_pos.x / 100.0), 0.0, 0.0, 1.0);
    let pixel_pos = vec2i(pos.x / i32(common::BITS_PER_PIXEL), pos.y);
    let pixel = get_pixel(pixel_pos);
    // let pixel = get_pixel(vec2i(0));

    let colour = (pixel >> (common::BITS_PER_PIXEL - 1 - (u32(pos.x) % common::BITS_PER_PIXEL))) & 1u;

    // let boundary = pos.x % i32(common::BITS_PER_PIXEL) == 0;
    // return vec4<f32>(f32(colour), vec2f(pos % #TILE_SIZE) / vec2f(#TILE_SIZE), 0);
    return vec4<f32>(vec3f(colour), 0);
}
