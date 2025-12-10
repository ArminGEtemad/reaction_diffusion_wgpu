@group(0) @binding(0)
var rd_texture : texture_2d<f32>;

@group(0) @binding(1)
var rd_sampler : sampler;

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

fn color_pallette(u: f32, v: f32) -> vec3<f32> {

    // TODO make a nice color pallette!
    let a = vec3<f32>(0.1, 0.7, 0.6);
    let b = vec3<f32>(1.0, 0.5, 0.1);
    let c = vec3<f32>(0.8, 0.2, 0.9);
    let d = vec3<f32>(0.1, 0.1, 0.9);

    let cl = clamp(u - v, 0.0, 1.0);

    // TODO maybe the color pallette changes over time
    // but we need a uniform binding for time here
    return d + c * cos(2.0 * (a * cl + b));
}

@fragment
fn fs_main(in : VSOut) -> @location(0) vec4<f32> {
    let u_v = textureSampleLevel(rd_texture, rd_sampler, in.uv, 0.0).rg; // only two channels 
    let u = u_v.x;
    let v = u_v.y;

    let color_theme = color_pallette(u, v);
    return vec4<f32>(color_theme, 1.0);
}


