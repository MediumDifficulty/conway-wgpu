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


@group(0) @binding(0) var world: texture_storage_2d<rg32uint, read>;
@group(1) @binding(0) var<uniform> camera: CameraUniform;

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
    let pos = vec2i((vertex.tex_coords * camera.screen_resolution - camera.screen_resolution / 2.) * camera.zoom + camera.centre);

    let dims = textureDimensions(world) * 8;
    if (pos.x < 0 || pos.y < 0 || any(vec2u(pos) >= dims)) {
        return vec4f(0., 0., 0., 0.);
    }

    let pixel_pos = pos / 8;
    let pixel = textureLoad(world, pixel_pos).rg;
    let pixel_bit = pos % 8;

    var alive = false;
    if (pixel_bit.y < 3) {
        alive = ((pixel.g >> u32(pixel_bit.y * 8 + pixel_bit.x)) & 1u) == 1u;
    } else {
        alive = ((pixel.r >> u32((pixel_bit.y - 4) * 8 + pixel_bit.x)) & 1u) == 1u;
    }

    #ifdef DEBUG
        let checker = vec3f(((pixel_pos.x+pixel_pos.y) % 2 == 0)) * 0.05;
        return vec4<f32>(vec3<f32>(alive) * 0.95 + checker, 1.);
    #else
        return vec4<f32>(vec3<f32>(alive), 1.);
    #endif

}
