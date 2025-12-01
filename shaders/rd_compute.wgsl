// defining constants for Reaction Diffusion System
const DU : f32 = 0.19; // diffusion rate for substance U
const DV : f32 = 0.08; // diffusion rate for substance V
const FEED : f32 = 0.0345; // Feed rate of U
const KILL : f32 = 0.062; // V's killing rate

struct TimeUniform {
    dt: f32,
};

@group(0) @binding(0)
var<uniform> u_time : TimeUniform;

@group(0) @binding(1)
var src_texture : texture_2d<f32>; // read from this

@group(0) @binding(2)
var dst_texture : texture_storage_2d<rgba32float, write>;  // write to this

// sample a pixel from the input
fn read_u_v(texture: texture_2d<f32>, x_y: vec2<i32>) -> vec2<f32> {
    let dims = textureDimensions(texture);
    
    // bounds
    let x = clamp(x_y.x, 0, i32(dims.x) - 1);
    let y = clamp(x_y.y, 0, i32(dims.y) - 1);
    
    // read the pixel
    let c = textureLoad(texture, vec2<i32>(x, y), 0);
    return c.rg; // U V values
}

// laplacian 4 neighbor
fn laplacian(texture: texture_2d<f32>, x_y: vec2<i32>) -> vec2<f32> {
    let center = read_u_v(texture, x_y);
    let up = read_u_v(texture, x_y + vec2<i32>(0, -1));
    let down = read_u_v(texture, x_y + vec2<i32>(0, 1));
    let left = read_u_v(texture, x_y + vec2<i32>(-1, 0));
    let right = read_u_v(texture, x_y + vec2<i32>(1, 0));

    let laplace = (up + down + left + right) - 4.0 * center;
    return laplace;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid : vec3<u32>) { 
    let dims = textureDimensions(dst_texture);
    let dt = u_time.dt;

    // bounds
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    // pixel values and position and the calculating the diffusion
    let x_y = vec2<i32>(i32(gid.x), i32(gid.y));
    let u_v = read_u_v(src_texture, x_y);
    let lap_u_v = laplacian(src_texture, x_y);

    var u = u_v.x;
    var v = u_v.y;

    // numerical calculation of the differential equation 
    // then calculate the integral over time
    let du = DU * lap_u_v.x - u * v * v + FEED * (1.0 - u);
    let dv = DV * lap_u_v.y + u * v * v - (FEED + KILL) * v;
    u = clamp(u + du * dt, 0.0, 1.0);
    v = clamp(v + dv * dt, 0.0, 1.0);

    let u_v_res= vec4<f32>(u, v, 0.0, 1.0);
    textureStore(dst_texture, x_y, u_v_res);
}
