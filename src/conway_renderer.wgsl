struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

struct CameraUniform {
    screen_resolution: vec2<f32>,
    centre: vec2<f32>,
    zoom: f32
}


@group(0) @binding(0) var world: texture_storage_2d<r32uint, read>;
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
    let colour = textureLoad(world, vec2<i32>(vertex.tex_coords * vec2<f32>(textureDimensions(world)) + camera.centre)).r;
    return vec4<f32>(vec3<f32>(colour), 0.0);
    // return vec4<f32>(vertex.tex_coords, 0.0, 0.0);
}
