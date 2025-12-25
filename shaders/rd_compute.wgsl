// defining constants for Reaction Diffusion System
const DU : f32 = 0.18; // diffusion rate for substance U
const DV : f32 = 0.085; // diffusion rate for substance V
const FEED : f32 = 0.037; // Feed rate of U
const KILL : f32 = 0.061; // V's killing rate

struct TimeUniform {
    dt: f32,
};
@group(0) @binding(0)
var<uniform> u_time : TimeUniform;

@group(0) @binding(1)
var tex_n : texture_2d<f32>;

@group(0) @binding(2)
var tex_star : texture_2d<f32>;

@group(0) @binding(3)
var tex_out : texture_storage_2d<rgba32float, write>;

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

    let up_left = read_u_v(texture, x_y + vec2<i32>(-1, -1));
    let up_right = read_u_v(texture, x_y + vec2<i32>(1, -1));
    let down_left = read_u_v(texture, x_y + vec2<i32>(-1, 1));
    let down_right = read_u_v(texture, x_y + vec2<i32>(1, 1));

    let cross_term = 4.0 * (up + down + left + right); 
    let diag_term = up_left + up_right + down_left + down_right;


    let laplace = (cross_term + diag_term - 20.0 * center) / 6.0;
    // let old_laplace = (cross_term - 16.0 * center) / 4.0;
    return laplace;
}

// predictor
@compute @workgroup_size(16, 16)
fn main_predictor(@builtin(global_invocation_id) gid : vec3<u32>) {
    let dims = textureDimensions(tex_out); 
    let dt = u_time.dt;

    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let x_y = vec2<i32>(i32(gid.x), i32(gid.y));

    let u_v = read_u_v(tex_n, x_y);
    let lap_u_v = laplacian(tex_n, x_y);

    var u = u_v.x;
    var v = u_v.y;

    let du = DU * lap_u_v.x - u * v * v + FEED * (1.0 - u);
    let dv = DV * lap_u_v.y + u * v * v - (FEED + KILL) * v;

    u = clamp(u + du * dt, 0.0, 1.0);
    v = clamp(v + dv * dt, 0.0, 1.0);

    textureStore(tex_out, x_y, vec4<f32>(u, v, 0.0, 1.0));
}

// corrector
@compute @workgroup_size(16, 16)
fn main_corrector(@builtin(global_invocation_id) gid : vec3<u32>) {
    let dims = textureDimensions(tex_n);
    let dt = u_time.dt;

    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let x_y = vec2<i32>(i32(gid.x), i32(gid.y));


    let u_v_n = read_u_v(tex_n, x_y);
    let lap_n = laplacian(tex_n, x_y);

    var u_n = u_v_n.x;
    var v_n = u_v_n.y;

    let du1 = DU * lap_n.x - u_n * v_n * v_n + FEED * (1.0 - u_n);
    let dv1 = DV * lap_n.y + u_n * v_n * v_n - (FEED + KILL) * v_n;


    let u_v_star = read_u_v(tex_star, x_y);
    let lap_star = laplacian(tex_star, x_y);

    var u_star = u_v_star.x;
    var v_star = u_v_star.y;

    let du2 = DU * lap_star.x - u_star * v_star * v_star + FEED * (1.0 - u_star);
    let dv2 = DV * lap_star.y + u_star * v_star * v_star - (FEED + KILL) * v_star;

    let u_next = clamp(u_n + 0.5 * dt * (du1 + du2), 0.0, 1.0);
    let v_next = clamp(v_n + 0.5 * dt * (dv1 + dv2), 0.0, 1.0);

    textureStore(tex_out, x_y, vec4<f32>(u_next, v_next, 0.0, 1.0));
}
