// Compute shader for ant simulation

struct Ant {
    pos: vec2<f32>,
    direction: vec2<f32>,
    state: u32, // 0 = searching, 1 = returning
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

@group(0) @binding(0) var<storage, read_write> ants: array<Ant>;
@group(0) @binding(1) var<storage, read_write> pheromone_map: array<f32>;
@group(0) @binding(2) var<storage, read_write> food_map: array<f32>;
@group(0) @binding(3) var<storage, read> obstacle_map: array<u32>;
@group(0) @binding(4) var<uniform> params: SimulationParams;

const ANT_SPEED: f32 = 70.0;
const ANT_STEER_STRENGTH: f32 = 5.0;
const WANDER_STRENGTH: f32 = 0.25;
const SENSOR_ANGLE_RAD: f32 = 0.628318530718; // PI/5
const SENSOR_DISTANCE: f32 = 20.0;
const GRID_CELL_SIZE: f32 = 4.0;
const PHEROMONE_DROP_RATE: f32 = 2.5;
const PHEROMONE_EVAPORATION_RATE: f32 = 0.995;
const PHEROMONE_MAX_INTENSITY: f32 = 1.0;
const HOME_RADIUS: f32 = 20.0;
const PI: f32 = 3.141592653589793;

// Simple random number generator
var<private> rng_state: u32;

fn init_rng(seed: u32) {
    rng_state = seed;
}

fn random() -> f32 {
    rng_state = rng_state * 1103515245u + 12345u;
    return f32(rng_state & 0x7FFFFFFFu) / 2147483647.0;
}

fn random_angle() -> vec2<f32> {
    let angle = random() * 2.0 * PI;
    return vec2<f32>(cos(angle), sin(angle));
}

fn pos_to_grid_index(pos: vec2<f32>) -> u32 {
    let grid_x = u32((pos.x + params.width / 2.0) / GRID_CELL_SIZE);
    let grid_y = u32((pos.y + params.height / 2.0) / GRID_CELL_SIZE);
    
    if (grid_x >= params.grid_cols || grid_y >= params.grid_rows) {
        return 0u;
    }
    
    return grid_y * params.grid_cols + grid_x;
}

fn get_pheromone_at(pos: vec2<f32>) -> f32 {
    let idx = pos_to_grid_index(pos);
    if (idx < arrayLength(&pheromone_map)) {
        return pheromone_map[idx];
    }
    return 0.0;
}

fn deposit_pheromone(pos: vec2<f32>, amount: f32) {
    let idx = pos_to_grid_index(pos);
    if (idx < arrayLength(&pheromone_map)) {
        pheromone_map[idx] = min(pheromone_map[idx] + amount, PHEROMONE_MAX_INTENSITY);
    }
}

fn is_obstacle_at(pos: vec2<f32>) -> bool {
    let idx = pos_to_grid_index(pos);
    if (idx < arrayLength(&obstacle_map)) {
        return obstacle_map[idx] != 0u;
    }
    return true;
}

fn take_food_at(pos: vec2<f32>) -> bool {
    let idx = pos_to_grid_index(pos);
    if (idx < arrayLength(&food_map)) {
        if (food_map[idx] > 0.0) {
            food_map[idx] = 0.0;
            return true;
        }
    }
    return false;
}

fn rotate_vector(v: vec2<f32>, angle: f32) -> vec2<f32> {
    let cos_a = cos(angle);
    let sin_a = sin(angle);
    return vec2<f32>(
        v.x * cos_a - v.y * sin_a,
        v.x * sin_a + v.y * cos_a
    );
}

@compute @workgroup_size(64)
fn update_ants(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let ant_index = global_id.x;
    
    if (ant_index >= params.ant_count) {
        return;
    }
    
    // Initialize RNG with ant index and time-based seed
    init_rng(ant_index + u32(params.dt * 1000.0));
    
    var ant = ants[ant_index];
    
    // Calculate sensor directions
    let forward_dir = normalize(ant.direction);
    let left_dir = rotate_vector(forward_dir, SENSOR_ANGLE_RAD);
    let right_dir = rotate_vector(forward_dir, -SENSOR_ANGLE_RAD);
    
    // Calculate sensor positions
    let sense_pos_fwd = ant.pos + forward_dir * SENSOR_DISTANCE;
    let sense_pos_left = ant.pos + left_dir * SENSOR_DISTANCE;
    let sense_pos_right = ant.pos + right_dir * SENSOR_DISTANCE;
    
    // Get pheromone intensities
    let intensity_fwd = get_pheromone_at(sense_pos_fwd);
    let intensity_left = get_pheromone_at(sense_pos_left);
    let intensity_right = get_pheromone_at(sense_pos_right);
    
    // Calculate desired direction
    let wander_dir = normalize(forward_dir + random_angle() * WANDER_STRENGTH);
    var desired_direction = wander_dir;
    
    if (intensity_fwd > intensity_left && intensity_fwd > intensity_right) {
        desired_direction = forward_dir;
    } else if (intensity_left > intensity_right) {
        desired_direction = left_dir;
    } else if (intensity_right > intensity_left) {
        desired_direction = right_dir;
    }
    
    // Update direction
    ant.direction = normalize(ant.direction + desired_direction * ANT_STEER_STRENGTH * params.dt);
    
    // Update position
    ant.pos += ant.direction * ANT_SPEED * params.dt;
    
    // Handle boundaries
    let half_w = params.width / 2.0;
    let half_h = params.height / 2.0;
    let margin = 5.0;
    
    if (ant.pos.x < -half_w + margin || ant.pos.x > half_w - margin) {
        ant.direction.x *= -1.0;
    }
    if (ant.pos.y < -half_h + margin || ant.pos.y > half_h - margin) {
        ant.direction.y *= -1.0;
    }
    
    if (is_obstacle_at(ant.pos)) {
        ant.pos -= ant.direction * 1.5;
        ant.direction *= -1.0;
    }
    
    ant.pos = clamp(ant.pos, vec2<f32>(-half_w, -half_h), vec2<f32>(half_w, half_h));
    
    // Handle state changes
    if (ant.state == 0u) { // Searching
        if (take_food_at(ant.pos)) {
            ant.state = 1u; // Returning
            ant.direction *= -1.0;
        }
    } else { // Returning
        let home_pos = vec2<f32>(0.0, 0.0);
        if (distance(ant.pos, home_pos) < HOME_RADIUS) {
            ant.state = 0u; // Searching
            ant.direction *= -1.0;
        }
    }
    
    // Drop pheromones (magenta trail)
    ant.pheromone_drop_timer += params.dt;
    if (ant.pheromone_drop_timer >= 1.0 / PHEROMONE_DROP_RATE) {
        deposit_pheromone(ant.pos, PHEROMONE_MAX_INTENSITY);
        ant.pheromone_drop_timer = 0.0;
    }
    
    ants[ant_index] = ant;
}

@compute @workgroup_size(64)
fn update_pheromones(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    if (index >= arrayLength(&pheromone_map)) {
        return;
    }
    
    // Apply pheromone decay
    pheromone_map[index] *= PHEROMONE_EVAPORATION_RATE;
}