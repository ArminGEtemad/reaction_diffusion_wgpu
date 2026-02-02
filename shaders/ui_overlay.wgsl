struct UiParams {
    // 1 = left, 2 = right, 3 = both
    active_side: u32,
}

@group(0) @binding(0)
var<uniform> u_ui : UiParams;

struct VSOut {
    @builtin(position) pos : vec4<f32>,
    @location(0) uv : vec2<f32>,
};

// helper function
fn geometric_logic(min: f32, max:f32, smoothness: f32, axis:f32) -> f32 {
    // s is 0 for everything left of min and 1 for right of min
    let s = smoothstep(min, min + smoothness, axis);

    // invert of s 
    let inv_s = 1.0 - smoothstep(max - smoothness, max, axis);

    // multiplication as in && giving us smooth edges on left and right
    return s * inv_s;
}

@vertex
fn vs_main(@builtin(vertex_index) vid : u32) -> VSOut {
    // full-screen triangle
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );

    var out : VSOut;
    let p = pos[vid];
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = p * 0.5 + 0.5; // [-1,1] -> [0,1]
    return out;
}

@fragment
fn fs_main(in : VSOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Panel bounds on the right side
    let panel_x_min = 0.9;
    let panel_x_max = 1.0;
    let panel_y_min = 0.95;
    let panel_y_max = 1.0;

    let x_mid = (panel_x_min + panel_x_max) * 0.5;

    let y_margin = 0.005;
    let x_margin = 0.002;

    let y_min = panel_y_min + y_margin;
    let y_max = panel_y_max - y_margin;
    let x_min = panel_x_min + x_margin;
    let x_max = panel_x_max - x_margin;

    let split_left = x_mid - x_margin * 0.8;
    let split_right = x_mid + x_margin * 0.8;

    // discarding whatever is not within
    if (uv.x < panel_x_min || uv.x > panel_x_max ||
        uv.y < panel_y_min || uv.y > panel_y_max) {
        discard;
    }

    let aa = max(fwidth(uv.x), fwidth(uv.y));; // smoothness is 1 pixel
    let in_rect_x = geometric_logic(x_min, x_max, aa, uv.x);
    let in_rect_y = geometric_logic(y_min, y_max, aa, uv.y);
    let is_divider = geometric_logic(split_left, split_right, aa, uv.x);

    // continuous smoothing in 2D
    let content_mask = in_rect_x * in_rect_y;

    // if not in content or in divider we are in border
    let border_strength = max((1.0 - content_mask), is_divider);

    // masks for the zones
    let left_zone_mask = 1.0 - smoothstep(split_left - aa, split_left, uv.x);
    let right_zone_mask = smoothstep(split_right, split_right + aa, uv.x);

    var active_intensity = 0.0;

    if (u_ui.active_side == 1u) { 
        active_intensity = left_zone_mask; 
    } else if (u_ui.active_side == 2u) { 
        active_intensity = right_zone_mask; 
    } else if (u_ui.active_side == 3u) { 
        active_intensity = max(left_zone_mask, right_zone_mask); 
    }

    // colors
    var base_color = vec3<f32>(0.02, 0.02, 0.02);
    var base_alpha = 1.0;
    var color_around = vec3<f32>(0.1, 0.1, 0.1);
    var alpha_around = 1.0;
    var color_active = vec3<f32>(0.0, 0.7, 0.0);
    var alpha_active = 1.0;

    // default color
    var final_color = base_color;
    var final_alpha = base_alpha;

    // Mix in the Active Glow
    // If active_intensity is 1.0, we get full green. If 0.0, base color.
    // The smoothstep makes this transition soft at the divider.
    final_color = mix(final_color, color_active, active_intensity);

    // Mix in the Border on top
    // If border_strength is 1.0, we get grey. This covers up the green glow at the edges.
    final_color = mix(final_color, color_around, border_strength);

    return vec4<f32>(final_color.rgb, final_alpha);
}
