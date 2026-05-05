struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) fg_color: vec4<f32>,
    @location(3) bg_color: vec4<f32>,
    @location(4) is_bg: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) is_bg: f32,
};

struct Uniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Convert pixel coordinates to clip space (-1..1)
    let x = (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let y = 1.0 - (in.position.y / uniforms.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = in.uv;
    out.fg_color = in.fg_color;
    out.bg_color = in.bg_color;
    out.is_bg = in.is_bg;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.is_bg > 0.5 {
        return in.bg_color;
    }

    let alpha = textureSample(atlas_texture, atlas_sampler, in.uv).r;
    if alpha < 0.01 {
        discard;
    }

    return vec4<f32>(in.fg_color.rgb, in.fg_color.a * alpha);
}
