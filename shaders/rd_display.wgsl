struct DisplayParameters {
    split_screen: u32,
}

@group(0) @binding(0)
var rd1_texture : texture_2d<f32>;

@group(0) @binding(1)
var rd2_texture : texture_2d<f32>;

@group(0) @binding(2)
var rd_sampler : sampler;

@group(0) @binding(3)
var<uniform> u_display : DisplayParameters;

struct VSOut {
    @builtin(position) pos : vec4<f32>,
    // screen space not UV elements reaction
    @location(0) uv : vec2<f32>, 
};

@vertex
fn vs_main(@builtin(vertex_index) vid : u32) -> VSOut {
    var pos = array<vec2<f32>, 3> (
        // one oversized triangle
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );

    var out : VSOut;
    let p = pos[vid];
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = p * 0.5 + 0.5; // [-1, +1] -> [0, +1]
    return out;
}

fn palette(t: f32, a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return a + b * cos(6.2831853 * (c * t + d)); // 2π = 6.2831853
}

fn color_pallette(u: f32, v: f32) -> vec3<f32> {
    // map the RD field to [0,1] smoothly
    let s = (u - v) * 1.5;
    let t = fract(0.5 + 0.5 * s);

    let a = vec3<f32>(0.2, 0.5, 0.5);
    let b = vec3<f32>(0.3, 0.5, 0.1);
    let c = vec3<f32>(1.0, 1.0, 1.0);
    let d = vec3<f32>(0.3, 0.3, 0.67);

    return palette(t, a, b, c, d);
}

@fragment
fn fs_main(in : VSOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // single-view mode: show rd1 full screen
    if (u_display.split_screen == 0u) {
        let u_v = textureSampleLevel(rd1_texture, rd_sampler, uv, 0.0).rg;
        let u = u_v.x;
        let v = u_v.y;
        let color_theme = color_pallette(u, v);

        return vec4<f32>(color_theme, 1.0);
    }

    // else 
    // add boundary between the screens
    let boundary_width = 0.001;
    if (abs(uv.x - 0.5) < boundary_width) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }

    // split-screen mode
    var sample_uv : vec2<f32>;
    var u_v : vec2<f32>;

    if (uv.x < 0.5) {
        // left half: rd1, remap x from [0, 0.5] -> [0, 1]
        let uv_left = vec2<f32>(uv.x * 2.0, uv.y);
        u_v = textureSampleLevel(rd1_texture, rd_sampler, uv_left, 0.0).rg;
    } else {
        // right half: rd2, remap x from [0.5, 1.0] -> [0, 1]
        let uv_right = vec2<f32>((uv.x - 0.5) * 2.0, uv.y);
        u_v = textureSampleLevel(rd2_texture, rd_sampler, uv_right, 0.0).rg;
    }

    let u = u_v.x;
    let v = u_v.y;
    let color_theme = color_pallette(u, v);

    return vec4<f32>(color_theme, 1.0);
}


