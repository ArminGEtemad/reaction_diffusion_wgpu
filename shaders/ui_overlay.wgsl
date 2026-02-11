const EPS: f32 = 0.001;

struct UiParams {
    active_side: u32, // 1 = left, 2 = right, 3 = both
    pause: u32,
    brush_radius: f32,
    brush_mode: u32,
    left_starting_pattern: u32, // 1 = circle 2 = square 3 = cleansheet
    right_starting_pattern: u32,
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

// circle AA
fn circle_aa(radius: f32, dist: f32, smoothness: f32) -> f32 {
    return smoothstep(radius - smoothness, radius + smoothness, dist);
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
    let screen_ratio = fwidth(uv.y) / fwidth(uv.x);
    let uv_corr = vec2<f32>(uv.x * screen_ratio, uv.y);
    let u_ui_min = 0.9;
    let aa = max(fwidth(uv.x), fwidth(uv.y));

    if (uv.x < u_ui_min) {
        discard;
    }

    // Colors
    let background_ui_color = vec4<f32>(0.001, 0.001, 0.01, 1.0);
    var panel_color = vec4<f32>(0.1, 0.8, 0.0, 1.0); // can be changed to the pause color
    let border_color = vec4<f32>(0.1, 0.1, 0.1, 1.0);
    let pause_color = vec4<f32>(0.7, 0.0, 0.1, 1.0);
    var bush_preview_color = vec4<f32>(0.7, 0.7, 0.4, 1.0);
    let active_brush_mode_color = vec4<f32>(0.2, 1.0, 0.0, 1.0);
    var starting_patter_placholder = vec4<f32>(0.1, 0.01, 0.1, 1.0);

    // panel
    let panel_u_max = 1.0; let panel_u_min = u_ui_min;
    let panel_v_max = 1.0; let panel_v_min = 0.9;
    let u_panel_mask = geometric_logic(panel_u_min, panel_u_max, aa, uv.x);
    let v_panel_mask = geometric_logic(panel_v_min, panel_v_max, aa, uv.y);
    let total_area_mask = u_panel_mask * v_panel_mask;

    // panel area
    let u_margin = 0.003;
    let v_margin = 0.005;
    let vg_min = panel_v_min + v_margin;
    let vg_max = panel_v_max - v_margin;
    let panel_split_mid = center_finder(panel_u_min, panel_u_max);
    
    // left area inside panel
    let leftg_u_min = panel_u_min + u_margin;
    let leftg_u_max = panel_split_mid - u_margin * 0.5;
    let leftg_u_mask = geometric_logic(leftg_u_min, leftg_u_max, aa, uv.x);
    let leftg_v_mask = geometric_logic(vg_min, vg_max, aa, uv.y);
    let leftg_mask = leftg_u_mask * leftg_v_mask;

    // right area inside panel
    let rightg_u_min = panel_split_mid + (u_margin*0.5);
    let rightg_u_max = panel_u_max - u_margin;
    let rightg_u_mask = geometric_logic(rightg_u_min, rightg_u_max, aa, uv.x);
    let rightg_v_mask = geometric_logic(vg_min, vg_max, aa, uv.y);
    let rightg_mask = rightg_u_mask * rightg_v_mask;

    // combining two areas inside panel areas with OR
    // whole area is left OR right inside panel
    let combined_glow_mask = max(leftg_mask, rightg_mask);
    
    // The border mask is the total area MINUS the glow areas
    let border_mask = clamp(total_area_mask - combined_glow_mask, 0.0, 1.0);

    // lamp
    let lamp_intensity = 0.001;
    let cu_left = center_finder(leftg_u_max, leftg_u_min);
    let cu_right = center_finder(rightg_u_max, rightg_u_min);
    let cy = center_finder(vg_max, vg_min);
    let c_left = vec2<f32>(cu_left*screen_ratio, cy);
    let c_right = vec2<f32>(cu_right*screen_ratio, cy);
    let dist_left = distance(uv_corr, c_left);
    let dist_right = distance(uv_corr, c_right);

    // glow
    // with inverse square law
    let glow_left = lamp_intensity / (dist_left * dist_left + EPS);
    let glow_right = lamp_intensity / (dist_right * dist_right + EPS);
    let both_glow = max(glow_left, glow_right);

    // colors
    var final_color = background_ui_color;
    var panel_switch = 0.0;
    var active_glow = 0.0;

    // Pause Play logic
    if (u_ui.pause != 0u) {
        panel_color = pause_color;
        panel_switch = combined_glow_mask;
    } else {
        if (u_ui.active_side == 1u) {
            panel_switch = leftg_mask;
            active_glow = glow_left;
        } else if (u_ui.active_side == 2u) {
            panel_switch = rightg_mask;
            active_glow = glow_right;
        } else if (u_ui.active_side == 3u) {
            panel_switch = combined_glow_mask;

            active_glow = both_glow; // Both contribute
        }
    }

    // Panel and border colored first
    final_color = mix(final_color, border_color, border_mask);

    // inside panel
    final_color = mix(final_color, panel_color, panel_switch);
    final_color += (panel_color * active_glow);

    // starting pattern preview
    let centers = array<vec2<f32>, 2>(
        vec2<f32>(cu_left * screen_ratio, 0.75),
        vec2<f32>(cu_right * screen_ratio, 0.75)
    );

    let patterns = array<u32, 2>(
        u_ui.left_starting_pattern,
        u_ui.right_starting_pattern
    );

    let r = 0.03;

    for (var i = 0u; i < 2u; i++) {
        let p = uv_corr - centers[i];
        let pattern_type = patterns[i];
        
        // circle and square mask
        let circ = 1.0 - circle_aa(r, length(p), aa);
        let square = 1.0 - smoothstep(r, r + aa, max(abs(p.x), abs(p.y)));
        
        // Determine which mask to use based on the pattern type
        var starting_patterm_mask = 0.0;
        if (pattern_type == 1u) {
            starting_patterm_mask = circ;
        } else if (pattern_type == 2u) {
            starting_patterm_mask = square;
        }
        
        // Apply to final color
        final_color = mix(final_color, starting_patter_placholder, starting_patterm_mask);
    }

    // half the screen for brush settings
    if (uv.y < 0.51 && uv.y > 0.49) {
        final_color = vec4<f32>(0.0, 0.1, 0.1, 1.0);
    }

    let c_brush= vec2<f32>(panel_split_mid * screen_ratio, 0.35);

    let uv_radius = u_ui.brush_radius*screen_ratio*fwidth(uv.x);
    let sigma = uv_radius / 2.0;
    let dist_brush = distance(uv_corr, c_brush);
    let gaussian_intensity = exp(- (dist_brush * dist_brush) / (2.0 * sigma * sigma));

    final_color = mix(final_color, bush_preview_color, gaussian_intensity);

    // different modes of the brush
    // three mode and three lamps
    // configuration
    let spacing = 0.07;
    let c_brush_mode_lamp_0 = vec2<f32>(panel_split_mid * screen_ratio, 0.15);
    let brush_mode_lamp_radius = 0.015;

    for (var i: i32 = -1; i <= 1; i = i + 1) {
        // three lamps -1, 0, +1
        let offset = vec2<f32>(f32(i) * spacing, 0.0);
        let dist_brush_mode = distance(uv_corr, c_brush_mode_lamp_0 + offset);
        let circ_brush_mode = 1.0 - circle_aa(brush_mode_lamp_radius, dist_brush_mode, aa);
        var brush_mode_color = vec4<f32>(0.5, 0.5, 0.5, 1.0);
        // TODO: get rid of if statements
        if (u_ui.brush_mode == 1u && i == -1) {
            brush_mode_color = active_brush_mode_color;
        } else if (u_ui.brush_mode == 2u && i == 0) {
            brush_mode_color = active_brush_mode_color;
        } else if (u_ui.brush_mode == 0u && i == 1) {
            brush_mode_color = active_brush_mode_color;
        }
        final_color = mix(final_color, brush_mode_color, circ_brush_mode);
    }

    return vec4<f32>(final_color);
}