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

// helper function for the mask
fn geometric_logic(min: f32, max:f32, smoothness: f32, axis:f32) -> f32 {
    // s is 0 for everything left of min and 1 for right of min
    let s = smoothstep(min, min + smoothness, axis);

    // invert of s 
    let inv_s = 1.0 - smoothstep(max - smoothness, max, axis);

    // multiplication as in && giving us smooth edges on left and right
    return s * inv_s;
}

// helper function to find centers
fn center_finder(lim_0: f32, lim_1: f32) -> f32 {
    return (lim_0 + lim_1) * 0.5;
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
    let x_ui_min = 0.9;
    let aa = max(fwidth(uv.x), fwidth(uv.y));

    if (uv.x < x_ui_min) {
        discard;
    }

    // Colors
    let background_ui_color = vec4<f32>(0.001, 0.001, 0.01, 1.0);
    let panel_color = vec4<f32>(0.3, 0.8, 0.0, 1.0);
    let border_color = vec4<f32>(0.1, 0.1, 0.1, 1.0);
    let lamp_color = vec4<f32>(0.9, 0.7, 0.2, 0.7);

    // panel
    let panel_x_max = 1.0; let panel_x_min = x_ui_min;
    let panel_y_max = 1.0; let panel_y_min = 0.9;
    let x_panel_mask = geometric_logic(panel_x_min, panel_x_max, aa, uv.x);
    let y_panel_mask = geometric_logic(panel_y_min, panel_y_max, aa, uv.y);
    let total_area_mask = x_panel_mask * y_panel_mask;

    // panel glow area
    let x_margin = 0.003;
    let y_margin = 0.005;
    let yg_min = panel_y_min + y_margin;
    let yg_max = panel_y_max - y_margin;
    let panel_split_mid = center_finder(panel_x_min, panel_x_max);
    
    // left glow area
    let leftg_x_min = panel_x_min + x_margin;
    let leftg_x_max = panel_split_mid - x_margin * 0.5;
    let leftg_x_mask = geometric_logic(leftg_x_min, leftg_x_max, aa, uv.x);
    let leftg_y_mask = geometric_logic(yg_min, yg_max, aa, uv.y);
    let leftg_mask = leftg_x_mask * leftg_y_mask;

    // right glow area
    let rightg_x_min = panel_split_mid + (x_margin*0.5);
    let rightg_x_max = panel_x_max - x_margin;
    let rightg_x_mask = geometric_logic(rightg_x_min, rightg_x_max, aa, uv.x);
    let rightg_y_mask = geometric_logic(yg_min, yg_max, aa, uv.y);
    let rightg_mask = rightg_x_mask * rightg_y_mask;

    // combining two glow areas with OR
    // whole glow area is left OR right
    let combined_glow_mask = max(leftg_mask, rightg_mask);
    
    // The border mask is the total area MINUS the glow areas
    let border_mask = clamp(total_area_mask - combined_glow_mask, 0.0, 1.0);

    // lamp
    let cx_left = center_finder(leftg_x_max, leftg_x_min);
    let cx_right = center_finder(rightg_x_max, rightg_x_min);
    let cy = center_finder(yg_max, yg_min);
    let c_left = vec2<f32>(cx_left, cy);
    let c_right = vec2<f32>(cx_right, cy);
    let dist_left = distance(uv, c_left);
    let dist_right = distance(uv, c_right);
    let r = 0.005;
    let circ_mask_left = max(exp(-100.0*(dist_left - r)), (1.0 - smoothstep(r-aa, r+aa, dist_left)));
    let circ_mask_right = max(exp(-100.0*(dist_right - r)), (smoothstep(r+aa, r-aa, dist_right)));
    let circ_mask_clamped_left = clamp(circ_mask_left, 0.0, 1.0);
    let circ_mask_clamped_right = clamp(circ_mask_right, 0.0, 1.0);

    var final_color = background_ui_color;
    var panel_switch = 0.0;
    var lamp_switch = 0.0;

    if (u_ui.active_side == 1u) {
        panel_switch = leftg_mask;
        lamp_switch = circ_mask_clamped_left;
    } else if (u_ui.active_side == 2u) {
        panel_switch = rightg_mask;
        lamp_switch = circ_mask_clamped_right;
    } else if (u_ui.active_side == 3u) {
        panel_switch = combined_glow_mask;
        lamp_switch = max(circ_mask_clamped_left, circ_mask_clamped_right);
    }

    final_color = mix(final_color, panel_color, panel_switch);
    final_color = mix(final_color, border_color, border_mask);
    final_color = mix(final_color, lamp_color, lamp_switch);

    return vec4<f32>(final_color);
}