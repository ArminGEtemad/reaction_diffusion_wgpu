struct DisplayParameters {
    split_screen: u32,
}

@group(0) @binding(0)
var rd1_texture : texture_2d<f32>;   // now COLOR
@group(0) @binding(1)
var rd2_texture : texture_2d<f32>;   // now COLOR
@group(0) @binding(2)
var rd_sampler : sampler;
@group(0) @binding(3)
var<uniform> u_display : DisplayParameters;

struct VSOut {
    @builtin(position) pos : vec4<f32>,
    @location(0) uv : vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid : u32) -> VSOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );

    var out : VSOut;
    let p = pos[vid];
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = p * 0.5 + 0.5;
    return out;
}

@fragment
fn fs_main(in : VSOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // split-screen disabled: rd1 full screen (already shaded color)
    if (u_display.split_screen == 0u) {
        let col = textureSampleLevel(rd1_texture, rd_sampler, uv, 0.0);
        return vec4<f32>(col.rgb, 1.0);
    }

    // split-screen: draw white boundary
    let boundary_width = 0.001;
    if (abs(uv.x - 0.5) < boundary_width) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }

    if (uv.x < 0.5) {
        let uv_left = vec2<f32>(uv.x * 2.0, uv.y);
        let col = textureSampleLevel(rd1_texture, rd_sampler, uv_left, 0.0);
        return vec4<f32>(col.rgb, 1.0);
    } else {
        let uv_right = vec2<f32>((uv.x - 0.5) * 2.0, uv.y);
        let col = textureSampleLevel(rd2_texture, rd_sampler, uv_right, 0.0);
        return vec4<f32>(col.rgb, 1.0);
    }
}
