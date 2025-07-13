// Render shader for ant simulation

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Ant {
    pos: vec2<f32>,
    direction: vec2<f32>,
    state: u32,
    pheromone_drop_timer: f32,
}

struct SimulationParams {
    width: f32,
    height: f32,
    dt: f32,
    grid_cols: u32,
    grid_rows: u32,
    ant_count: u32,
}

@group(0) @binding(0) var<storage, read> ants: array<Ant>;
@group(0) @binding(1) var<storage, read> pheromone_map: array<f32>;
@group(0) @binding(2) var<storage, read> food_map: array<f32>;
@group(0) @binding(3) var<storage, read> obstacle_map: array<u32>;
@group(0) @binding(4) var<uniform> params: SimulationParams;

const HOME_RADIUS: f32 = 20.0;
const GRID_CELL_SIZE: f32 = 4.0;

// Vertex shader for fullscreen quad
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Create a fullscreen quad using vertex index
    let x = f32(i32(vertex_index & 1u) * 2 - 1);
    let y = f32(i32(vertex_index >> 1u) * 2 - 1);
    
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, y * 0.5 + 0.5);
    
    return out;
}

fn uv_to_world_pos(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        (uv.x - 0.5) * params.width,
        (uv.y - 0.5) * params.height
    );
}

fn world_pos_to_grid_index(pos: vec2<f32>) -> u32 {
    let grid_x = u32((pos.x + params.width / 2.0) / GRID_CELL_SIZE);
    let grid_y = u32((pos.y + params.height / 2.0) / GRID_CELL_SIZE);
    
    if (grid_x >= params.grid_cols || grid_y >= params.grid_rows) {
        return 0u;
    }
    
    return grid_y * params.grid_cols + grid_x;
}

fn get_pheromone_intensity(pos: vec2<f32>) -> f32 {
    let idx = world_pos_to_grid_index(pos);
    if (idx < arrayLength(&pheromone_map)) {
        return pheromone_map[idx];
    }
    return 0.0;
}

fn get_food_at(pos: vec2<f32>) -> f32 {
    let idx = world_pos_to_grid_index(pos);
    if (idx < arrayLength(&food_map)) {
        return food_map[idx];
    }
    return 0.0;
}

fn is_obstacle_at(pos: vec2<f32>) -> bool {
    let idx = world_pos_to_grid_index(pos);
    if (idx < arrayLength(&obstacle_map)) {
        return obstacle_map[idx] != 0u;
    }
    return false;
}

fn distance_to_ant(world_pos: vec2<f32>) -> f32 {
    var min_dist = 999999.0;
    
    for (var i = 0u; i < params.ant_count; i++) {
        let ant = ants[i];
        let dist = distance(world_pos, ant.pos);
        min_dist = min(min_dist, dist);
    }
    
    return min_dist;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert UV to world position
    let world_pos = uv_to_world_pos(in.uv);
    
    // Background color
    var color = vec3<f32>(0.12, 0.16, 0.22); // Dark blue-gray background
    
    // Render pheromone trail as magenta
    let pheromone_intensity = get_pheromone_intensity(world_pos);
    if (pheromone_intensity > 0.01) {
        // Magenta color (red + blue components)
        let magenta_color = vec3<f32>(1.0, 0.0, 1.0);
        color = mix(color, magenta_color, pheromone_intensity * 0.7);
    }
    
    // Render food
    let food_amount = get_food_at(world_pos);
    if (food_amount > 0.0) {
        let food_color = vec3<f32>(0.2, 0.83, 0.6); // Green
        color = mix(color, food_color, food_amount);
    }
    
    // Render obstacles
    if (is_obstacle_at(world_pos)) {
        color = vec3<f32>(0.3, 0.34, 0.39); // Gray
    }
    
    // Render home area
    let home_pos = vec2<f32>(0.0, 0.0);
    if (distance(world_pos, home_pos) < HOME_RADIUS) {
        let home_color = vec3<f32>(0.38, 0.65, 0.98); // Light blue
        color = mix(color, home_color, 0.6);
    }
    
    // Render ants on top
    let ant_dist = distance_to_ant(world_pos);
    if (ant_dist < 3.0) {
        // Find the closest ant to determine its state
        var closest_ant_state = 0u;
        var closest_dist = 999999.0;
        
        for (var i = 0u; i < params.ant_count; i++) {
            let ant = ants[i];
            let dist = distance(world_pos, ant.pos);
            if (dist < closest_dist) {
                closest_dist = dist;
                closest_ant_state = ant.state;
            }
        }
        
        // Ant colors
        let ant_color = select(
            vec3<f32>(1.0, 1.0, 1.0),    // White for searching
            vec3<f32>(0.98, 0.75, 0.14), // Yellow for returning
            closest_ant_state == 1u
        );
        
        let ant_alpha = smoothstep(3.0, 1.0, ant_dist);
        color = mix(color, ant_color, ant_alpha);
    }
    
    return vec4<f32>(color, 1.0);
}