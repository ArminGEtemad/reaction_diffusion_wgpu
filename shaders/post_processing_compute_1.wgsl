@group(0) @binding(0)
var rd_in : texture_2d<f32>;

@group(0) @binding(1)
var rd_out : texture_storage_2d<rgba32float, write>;

fn palette(t: f32, a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return a + b * cos(6.2831853 * (c * t + d));
}

fn color_pallette(u: f32, v: f32) -> vec3<f32> {
    let s = (u - v) * 1.5;
    let t = fract(0.5 + 0.5 * s);

    let a = vec3<f32>(0.05, 0.00, 0.00);
    let b = vec3<f32>(1.20, 0.30, 0.00);
    let c = vec3<f32>(1.00, 0.80, 0.50);
    let d = vec3<f32>(0.00, 0.20, 0.70);

    return palette(t, a, b, c, d);
}


fn height_from_uv(u: f32, v: f32) -> f32 {
    return u - v; // contrast between the elemets 
}

fn clamp_coord(p: vec2<i32>, dims: vec2<i32>) -> vec2<i32> {
    return vec2<i32>(
        clamp(p.x, 0, dims.x - 1),
        clamp(p.y, 0, dims.y - 1),
    );
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid : vec3<u32>) {
    let dims_u = textureDimensions(rd_in);
    if (gid.x >= dims_u.x || gid.y >= dims_u.y) {
        return;
    }

    let dims  = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let coord = vec2<i32>(i32(gid.x), i32(gid.y));

    // center sample
    let center  = textureLoad(rd_in, coord, 0);
    let u_center = center.r; // u element center
    let v_center = center.g; // v element center

    let base_color = color_pallette(u_center, v_center);

    // neighbors for height gradient
    let left = textureLoad(rd_in, clamp_coord(coord + vec2<i32>(-1, 0), dims), 0);
    let right = textureLoad(rd_in, clamp_coord(coord + vec2<i32>(1, 0), dims), 0);
    let down = textureLoad(rd_in, clamp_coord(coord + vec2<i32>(0, -1), dims), 0);
    let up = textureLoad(rd_in, clamp_coord(coord + vec2<i32>(0, 1), dims), 0);

    let h_left = height_from_uv(left.r, left.g);
    let h_right = height_from_uv(right.r, right.g);
    let h_down = height_from_uv(down.r, down.g);
    let h_up = height_from_uv(up.r, up.g);

    // gradient
    let dh_dx = h_right - h_left; // divided by 2 is absorbed later in slope
    let dh_dy = h_up - h_down; // divided by 2 is absorbed later in slope

    let slope_scale = 5.0;
    let n = normalize(vec3<f32>(
        -dh_dx * slope_scale,
        -dh_dy * slope_scale,
        1.0,
    ));

    // same light direction
    let light_dir = normalize(vec3<f32>(0.9, 0.4, 0.8));
    let diffuse = clamp(dot(n, light_dir), 0.0, 1.0);
    let ambient = 0.0001;
    let shade = ambient + (1.0 - ambient) * diffuse;

    let shaded_color = base_color * shade;

    textureStore(rd_out, coord, vec4<f32>(shaded_color, 1.0));
}
