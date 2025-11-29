struct TimeUniform {
    dt: f32,
};

@group(0) @binding(0)
var<uniform> u_time : TimeUniform;

@compute @workgroup_size(256) // has to be changed later for 2D 
fn main(@builtin(global_invocation_id) gid : vec3<u32>) { 
    // this is just a placeholder for now
    let t = u_time.dt;
} 

