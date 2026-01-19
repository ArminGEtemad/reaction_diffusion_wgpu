struct TimeUniform {
    dt: f32,
};

struct RDSystemParameters {
    du_rate: f32,
    dv_rate: f32,
    feed: f32,
    kill: f32,
}

// time uniform buffer
@group(0) @binding(0)
var<uniform> u_time : TimeUniform;

// source for tex_n
@group(0) @binding(1)
var tex_n : texture_2d<f32>;

// storage for calculating tex_star in stage 1 of RK2 and a source to read
// for stage 2 of RK2
@group(0) @binding(2)
var tex_star : texture_storage_2d<rgba32float, read_write>;

// end storage for result of RK2
@group(0) @binding(3)
var tex_out : texture_storage_2d<rgba32float, write>;

// uploading system parameters
@group(0) @binding(4)
var<uniform> sys_params : RDSystemParameters;

// helper function for RD systems
fn rd_system_rhs(u: f32, v: f32, lap: vec2<f32>) -> vec2<f32> {
    // derivatives of the coupled differential equations
    let du = sys_params.du_rate * lap.x - u * v * v + sys_params.feed * (1.0 - u);
    let dv = sys_params.dv_rate * lap.y + u * v * v - (sys_params.feed + sys_params.kill) * v;

    return vec2<f32>(du, dv);
}

// helper function for 9 point stencil
fn nine_point_stencil(
    center: vec2<f32>, up: vec2<f32>,
    down: vec2<f32>, left: vec2<f32>,
    right: vec2<f32>, up_left: vec2<f32>,
    up_right: vec2<f32>, down_left: vec2<f32>,
    down_right: vec2<f32>,
) -> vec2<f32> {
    let cross_term = 4.0 * (up + down + left + right);
    let diag_term = up_left + up_right + down_left + down_right;
    let laplace = (cross_term + diag_term - 20.0 * center) / 6.0;
    return laplace;
}

// sample a pixel from the input
// helper function to read get the data from source in binding 1
fn read_u_v(x_y: vec2<i32>) -> vec2<f32> {
    let dims = textureDimensions(tex_n);
    let x = clamp(x_y.x, 0, i32(dims.x) - 1);
    let y = clamp(x_y.y, 0, i32(dims.y) - 1);
    let c = textureLoad(tex_n, vec2<i32>(x, y), 0);
    return c.rg;
}

// helper function to get the data from temporary storage in binding 2
fn read_u_v_star(x_y: vec2<i32>) -> vec2<f32> {
    let dims = textureDimensions(tex_star);
    let x = clamp(x_y.x, 0, i32(dims.x) - 1);
    let y = clamp(x_y.y, 0, i32(dims.y) - 1);
    let c = textureLoad(tex_star, vec2<i32>(x, y));
    return c.rg;
}

// laplacian calculated using 9 point stencil
// calculates the laplacian from the source binding 1
fn laplacian(center: vec2<f32>, x_y: vec2<i32>) -> vec2<f32> {
    let up = read_u_v(x_y + vec2<i32>(0, -1));
    let down = read_u_v(x_y + vec2<i32>(0, 1));
    let left = read_u_v(x_y + vec2<i32>(-1, 0));
    let right = read_u_v(x_y + vec2<i32>(1, 0));

    let up_left = read_u_v(x_y + vec2<i32>(-1, -1));
    let up_right = read_u_v(x_y + vec2<i32>(1, -1));
    let down_left = read_u_v(x_y + vec2<i32>(-1, 1));
    let down_right = read_u_v(x_y + vec2<i32>(1, 1));

    let laplace = nine_point_stencil(
        center, up, down, left, right, 
        up_left, up_right, down_left, down_right
    );
    return laplace;
}

// calculates the laplacian from the storage binding 2
fn laplacian_star(center: vec2<f32>, x_y: vec2<i32>) -> vec2<f32> {
    let up = read_u_v_star(x_y + vec2<i32>(0, -1));
    let down = read_u_v_star(x_y + vec2<i32>(0, 1));
    let left = read_u_v_star(x_y + vec2<i32>(-1, 0));
    let right = read_u_v_star(x_y + vec2<i32>(1, 0));

    let up_left = read_u_v_star(x_y + vec2<i32>(-1, -1));
    let up_right = read_u_v_star(x_y + vec2<i32>(1, -1));
    let down_left = read_u_v_star(x_y + vec2<i32>(-1, 1));
    let down_right = read_u_v_star(x_y + vec2<i32>(1, 1));

    let laplace = nine_point_stencil(
        center, up, down, left, right, 
        up_left, up_right, down_left, down_right
    );
    return laplace;
}


// predictor
@compute @workgroup_size(16, 16)
fn main_predictor(@builtin(global_invocation_id) gid : vec3<u32>) {
    let dims = textureDimensions(tex_n); 
    let dt = u_time.dt;

    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }

    let x_y = vec2<i32>(i32(gid.x), i32(gid.y));

    let u_v = read_u_v(x_y);
    let lap_u_v = laplacian(u_v, x_y);

    var u = u_v.x;
    var v = u_v.y;

    // stage one solution before integration
    let rd_rhs = rd_system_rhs(u, v, lap_u_v); 

    let du = rd_rhs.x;
    let dv = rd_rhs.y;

    let u_star = clamp(u + du * dt, 0.0, 1.0);
    let v_star = clamp(v + dv * dt, 0.0, 1.0);

    textureStore(tex_star, x_y, vec4<f32>(u_star, v_star, 0.0, 1.0));
}


// corrector
@compute @workgroup_size(16, 16)
fn main_corrector(@builtin(global_invocation_id) gid : vec3<u32>) {
    let dims = textureDimensions(tex_star);
    let dt = u_time.dt;

    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let x_y = vec2<i32>(i32(gid.x), i32(gid.y));

    // slope at n
    let u_v_n = read_u_v(x_y);
    let lap_n = laplacian(u_v_n, x_y);

    var u_n = u_v_n.x;
    var v_n = u_v_n.y;

    let rd_rhs_n = rd_system_rhs(u_n, v_n, lap_n);

    let du1 = rd_rhs_n.x;
    let dv1 = rd_rhs_n.y;

    // slope at star
    let u_v_star = read_u_v_star(x_y);
    let lap_star = laplacian_star(u_v_star, x_y);

    var u_star = u_v_star.x;
    var v_star = u_v_star.y;

    let rd_rhs_star = rd_system_rhs(u_star, v_star, lap_star);

    let du2 = rd_rhs_star.x;
    let dv2 = rd_rhs_star.y;

    let u_next = clamp(u_n + 0.5 * dt * (du1 + du2), 0.0, 1.0);
    let v_next = clamp(v_n + 0.5 * dt * (dv1 + dv2), 0.0, 1.0);

    textureStore(tex_out, x_y, vec4<f32>(u_next, v_next, 0.0, 1.0));
}
