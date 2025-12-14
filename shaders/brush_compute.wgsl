// TODO: I need to put all consts somewhere and just take them from there
// hard coding is not the way :(
const WIDTH : u32 = 1280;
const HEIGHT : u32 = 1280;

// brush parameters
struct BrushUniform {
    c_x: f32,
    c_y: f32,
    radius: f32,
    mode: u32,
};

@group(0) @binding(0)
var<uniform> brush : BrushUniform;

@group(0) @binding(1)
var texture_s : texture_storage_2d<rgba32float, read_write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid : vec3<u32>) {
    let x_u32 = gid.x; 
    let y_u32 = gid.y;

    // bounds
    if x_u32 >= WIDTH || y_u32 >= HEIGHT { return; }

    // if inactive don't draw
    if brush.radius <= 0.0 { return; }


    let x_float32 = f32(x_u32);
    let y_float32 = f32(y_u32);
    let dist_x = x_float32 - brush.c_x;
    let dist_y = y_float32 - brush.c_y;

    let dist = sqrt(dist_x * dist_x + dist_y * dist_y);

    // Gaussian distribution because the brush behaves like one
    // just like the spherical cows in my theoretical physics courses
    let sigma = brush.radius / 2.0;
    let normal_distri = exp(- (dist * dist) / (2.0 * sigma * sigma));

    let coordinate = vec2<i32>(i32(x_u32), i32(y_u32));
    var c = textureLoad(texture_s, coordinate);

    let default_v = 0.0;
    let default_u = 1.0;

    switch brush.mode {
        // 0 add V
        case 0u: {
            c.y = c.y + normal_distri;
        }

        // case 1 add U
        case 1u: {
            c.x = c.x + normal_distri;
        }

        case 3u: {
            c.x = mix(c.x, default_u, normal_distri);
            c.y = mix(c.y, default_v, normal_distri);
        }

        default : {}
    }
    

    textureStore(texture_s, coordinate, c);    

}




