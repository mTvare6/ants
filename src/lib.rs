use glam::DVec2;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement};

fn random_angle() -> DVec2 {
    let angle = js_sys::Math::random() * 2.0 * PI;
    DVec2::from_angle(angle)
}

pub const ANT_COUNT: usize = 200;
pub const ANT_SPEED: f64 = 70.0;
pub const ANT_STEER_STRENGTH: f64 = 5.0;
pub const WANDER_STRENGTH: f64 = 0.25;
pub const SENSOR_ANGLE_RAD: f64 = PI / 5.0;
pub const SENSOR_DISTANCE: f64 = 20.0;
pub const GRID_CELL_SIZE: u32 = 4;
pub const PHEROMONE_DROP_RATE: f64 = 2.5;
pub const PHEROMONE_EVAPORATION_RATE: f64 = 0.995;
pub const PHEROMONE_MAX_INTENSITY: f64 = 1.0;

pub const HOME_RADIUS: f64 = 20.0;

#[derive(Clone, Copy, PartialEq)]
enum AntState {
    Searching,
    Returning,
}

struct Ant {
    pos: DVec2,
    direction: DVec2,
    state: AntState,
    pheromone_drop_timer: f64,
}

impl Ant {
    fn new(pos: DVec2) -> Self {
        Self {
            pos,
            direction: random_angle(),
            state: AntState::Searching,
            pheromone_drop_timer: 0.0,
        }
    }

    fn update(&mut self, dt: f64, world: &mut World) {
        let forward_dir = self.direction;
        let left_dir = DVec2::new(
            forward_dir.x * SENSOR_ANGLE_RAD.cos() - forward_dir.y * SENSOR_ANGLE_RAD.sin(),
            forward_dir.x * SENSOR_ANGLE_RAD.sin() + forward_dir.y * SENSOR_ANGLE_RAD.cos(),
        );
        let right_dir = DVec2::new(
            forward_dir.x * SENSOR_ANGLE_RAD.cos() + forward_dir.y * SENSOR_ANGLE_RAD.sin(),
            -forward_dir.x * SENSOR_ANGLE_RAD.sin() + forward_dir.y * SENSOR_ANGLE_RAD.cos(),
        );

        let sense_pos_fwd = self.pos + forward_dir * SENSOR_DISTANCE;
        let sense_pos_left = self.pos + left_dir * SENSOR_DISTANCE;
        let sense_pos_right = self.pos + right_dir * SENSOR_DISTANCE;

        let target_pheromone = match self.state {
            AntState::Searching => PheromoneType::ToFood,
            AntState::Returning => PheromoneType::ToHome,
        };

        let intensity_fwd = world.get_pheromone_at(sense_pos_fwd, target_pheromone);
        let intensity_left = world.get_pheromone_at(sense_pos_left, target_pheromone);
        let intensity_right = world.get_pheromone_at(sense_pos_right, target_pheromone);

        let wander_dir = (self.direction + random_angle() * WANDER_STRENGTH).normalize();
        let mut desired_direction = wander_dir;

        if intensity_fwd > intensity_left && intensity_fwd > intensity_right {
            desired_direction = forward_dir;
        } else if intensity_left > intensity_right {
            desired_direction = left_dir;
        } else if intensity_right > intensity_left {
            desired_direction = right_dir;
        }

        self.direction = (self.direction + desired_direction * ANT_STEER_STRENGTH * dt).normalize();

        self.pos += self.direction * ANT_SPEED * dt;

        self.handle_boundaries(world);
        self.interact_with_world(world);

        self.pheromone_drop_timer += dt;
        if self.pheromone_drop_timer >= 1.0 / PHEROMONE_DROP_RATE {
            let drop_type = match self.state {
                AntState::Searching => PheromoneType::ToHome,
                AntState::Returning => PheromoneType::ToFood,
            };
            world.deposit_pheromone(self.pos, drop_type, PHEROMONE_MAX_INTENSITY);
            self.pheromone_drop_timer = 0.0;
        }
    }

    fn interact_with_world(&mut self, world: &mut World) {
        match self.state {
            AntState::Searching => {
                if world.take_food_at(self.pos) {
                    self.state = AntState::Returning;
                    self.direction *= -1.0;
                }
            }
            AntState::Returning => {
                if self.pos.distance_squared(world.home_pos) < HOME_RADIUS * HOME_RADIUS {
                    self.state = AntState::Searching;
                    self.direction *= -1.0;
                }
            }
        }
    }

    fn handle_boundaries(&mut self, world: &World) {
        let half_w = world.width / 2.0;
        let half_h = world.height / 2.0;
        let margin = 5.0;

        if self.pos.x < -half_w + margin || self.pos.x > half_w - margin {
            self.direction.x *= -1.0;
        }
        if self.pos.y < -half_h + margin || self.pos.y > half_h - margin {
            self.direction.y *= -1.0;
        }

        if world.is_obstacle_at(self.pos) {
            self.pos -= self.direction * 1.5;
            self.direction *= -1.0;
        }

        self.pos = self
            .pos
            .clamp(DVec2::new(-half_w, -half_h), DVec2::new(half_w, half_h));
    }

    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        ctx.save();
        ctx.translate(self.pos.x, self.pos.y).unwrap();
        ctx.rotate(self.direction.y.atan2(self.direction.x))
            .unwrap();

        let color = match self.state {
            AntState::Searching => "#FFFFFF",
            AntState::Returning => "#FBBF24",
        };
        ctx.set_fill_style(&JsValue::from_str(color));

        ctx.begin_path();
        ctx.move_to(5.0, 0.0);
        ctx.line_to(-2.5, -3.0);
        ctx.line_to(-2.5, 3.0);
        ctx.close_path();
        ctx.fill();

        ctx.restore();
    }
}

#[derive(Clone, Copy)]
enum PheromoneType {
    ToHome,
    ToFood,
}

struct World {
    width: f64,
    height: f64,
    home_pos: DVec2,
    grid_cols: usize,
    grid_rows: usize,
    to_home_pheromones: Vec<f64>,
    to_food_pheromones: Vec<f64>,
    food: Vec<f64>,
    obstacles: Vec<bool>,

    offscreen_canvas: HtmlCanvasElement,
    offscreen_ctx: CanvasRenderingContext2d,
}

impl World {
    fn new(width: f64, height: f64, document: &Document) -> Result<Self, JsValue> {
        let grid_cols = (width / GRID_CELL_SIZE as f64) as usize;
        let grid_rows = (height / GRID_CELL_SIZE as f64) as usize;
        let grid_size = grid_cols * grid_rows;

        let offscreen_canvas = document
            .create_element("canvas")?
            .dyn_into::<HtmlCanvasElement>()?;
        offscreen_canvas.set_width(grid_cols as u32);
        offscreen_canvas.set_height(grid_rows as u32);
        let offscreen_ctx = offscreen_canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()?;

        let mut world = Self {
            width,
            height,
            home_pos: DVec2::ZERO,
            grid_cols,
            grid_rows,
            to_home_pheromones: vec![0.0; grid_size],
            to_food_pheromones: vec![0.0; grid_size],
            food: vec![0.0; grid_size],
            obstacles: vec![false; grid_size],
            offscreen_canvas,
            offscreen_ctx,
        };

        world.add_food_cluster(DVec2::new(300.0, 200.0), 50.0, 100);
        world.add_food_cluster(DVec2::new(-300.0, -200.0), 50.0, 100);
        world.add_food_cluster(DVec2::new(0.0, -250.0), 40.0, 70);

        let wall_start_y = -100.0;
        let wall_end_y = 100.0;
        let wall_x = 150.0;
        for y in (wall_start_y as i32)..(wall_end_y as i32) {
            if let Some(idx) = world.pos_to_grid_index(DVec2::new(wall_x, y as f64)) {
                world.obstacles[idx] = true;
            }
        }

        Ok(world)
    }

    fn pos_to_grid_index(&self, pos: DVec2) -> Option<usize> {
        let grid_x = ((pos.x + self.width / 2.0) / GRID_CELL_SIZE as f64) as isize;
        let grid_y = ((pos.y + self.height / 2.0) / GRID_CELL_SIZE as f64) as isize;

        if grid_x >= 0
            && grid_x < self.grid_cols as isize
            && grid_y >= 0
            && grid_y < self.grid_rows as isize
        {
            Some((grid_y as usize) * self.grid_cols + (grid_x as usize))
        } else {
            None
        }
    }

    fn add_food_cluster(&mut self, center: DVec2, radius: f64, count: u32) {
        for _ in 0..count {
            let pos = center + random_angle() * js_sys::Math::random() * radius;
            if let Some(idx) = self.pos_to_grid_index(pos) {
                self.food[idx] = 1.0;
            }
        }
    }

    fn get_pheromone_at(&self, pos: DVec2, p_type: PheromoneType) -> f64 {
        if let Some(idx) = self.pos_to_grid_index(pos) {
            return match p_type {
                PheromoneType::ToHome => self.to_home_pheromones[idx],
                PheromoneType::ToFood => self.to_food_pheromones[idx],
            };
        }
        0.0
    }

    fn deposit_pheromone(&mut self, pos: DVec2, p_type: PheromoneType, amount: f64) {
        if let Some(idx) = self.pos_to_grid_index(pos) {
            let pheromones = match p_type {
                PheromoneType::ToHome => &mut self.to_home_pheromones,
                PheromoneType::ToFood => &mut self.to_food_pheromones,
            };
            pheromones[idx] = (pheromones[idx] + amount).min(PHEROMONE_MAX_INTENSITY);
        }
    }

    fn take_food_at(&mut self, pos: DVec2) -> bool {
        if let Some(idx) = self.pos_to_grid_index(pos) {
            if self.food[idx] > 0.0 {
                self.food[idx] = 0.0;
                return true;
            }
        }
        false
    }

    fn is_obstacle_at(&self, pos: DVec2) -> bool {
        if let Some(idx) = self.pos_to_grid_index(pos) {
            return self.obstacles[idx];
        }
        true
    }

    fn update(&mut self) {
        for i in 0..self.to_home_pheromones.len() {
            self.to_home_pheromones[i] *= PHEROMONE_EVAPORATION_RATE;
            self.to_food_pheromones[i] *= PHEROMONE_EVAPORATION_RATE;
        }
    }

    fn draw_world_to_buffer(&self) {
        self.offscreen_ctx
            .set_fill_style(&JsValue::from_str("#1f2937"));
        self.offscreen_ctx
            .fill_rect(0.0, 0.0, self.grid_cols as f64, self.grid_rows as f64);

        self.offscreen_ctx.set_global_alpha(0.7);

        for y in 0..self.grid_rows {
            for x in 0..self.grid_cols {
                let idx = y * self.grid_cols + x;

                // Render pheromones as magenta trail
                if self.to_food_pheromones[idx] > 0.01 || self.to_home_pheromones[idx] > 0.01 {
                    // Combine both pheromone types for magenta trail
                    let combined_intensity = (self.to_food_pheromones[idx] + self.to_home_pheromones[idx]).min(1.0);
                    let alpha = combined_intensity;
                    self.offscreen_ctx
                        .set_fill_style(&JsValue::from_str(&format!(
                            "rgba(255, 0, 255, {})",
                            alpha
                        )));
                    self.offscreen_ctx.fill_rect(x as f64, y as f64, 1.0, 1.0);
                }

                if self.food[idx] > 0.0 {
                    self.offscreen_ctx
                        .set_fill_style(&JsValue::from_str("#34D399"));
                    self.offscreen_ctx.fill_rect(x as f64, y as f64, 1.0, 1.0);
                }

                if self.obstacles[idx] {
                    self.offscreen_ctx
                        .set_fill_style(&JsValue::from_str("#4B5563"));
                    self.offscreen_ctx.fill_rect(x as f64, y as f64, 1.0, 1.0);
                }
            }
        }
    }
}

struct Simulation {
    ants: Vec<Ant>,
    world: World,
}

impl Simulation {
    fn new(width: f64, height: f64, document: &Document) -> Result<Self, JsValue> {
        let world = World::new(width, height, document)?;
        let ants = (0..ANT_COUNT).map(|_| Ant::new(world.home_pos)).collect();

        Ok(Self { ants, world })
    }

    fn update(&mut self, dt: f64) {
        for ant in &mut self.ants {
            ant.update(dt, &mut self.world);
        }
        self.world.update();
    }

    fn draw(&self, ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
        self.world.draw_world_to_buffer();

        ctx.set_fill_style(&JsValue::from_str("#1f2937"));
        ctx.fill_rect(
            -self.world.width / 2.0,
            -self.world.height / 2.0,
            self.world.width,
            self.world.height,
        );

        ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
            &self.world.offscreen_canvas,
            -self.world.width / 2.0,
            -self.world.height / 2.0,
            self.world.width,
            self.world.height,
        )?;

        ctx.set_fill_style(&JsValue::from_str("#60A5FA"));
        ctx.begin_path();
        ctx.arc(
            self.world.home_pos.x,
            self.world.home_pos.y,
            HOME_RADIUS,
            0.0,
            2.0 * PI,
        )?;
        ctx.fill();

        for ant in &self.ants {
            ant.draw(ctx);
        }

        Ok(())
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document.get_element_by_id("simulation-canvas").unwrap();
    let canvas: HtmlCanvasElement = canvas.dyn_into()?;

    let width = canvas.width() as f64;
    let height = canvas.height() as f64;

    let context = canvas
        .get_context("2d")?
        .unwrap()
        .dyn_into::<CanvasRenderingContext2d>()?;

    context.translate(width / 2.0, height / 2.0)?;

    let simulation = Rc::new(RefCell::new(Simulation::new(width, height, &document)?));

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    let mut last_time = window.performance().unwrap().now();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let now = web_sys::window().unwrap().performance().unwrap().now();
        let dt = (now - last_time) / 1000.0;
        last_time = now;

        let mut sim = simulation.borrow_mut();
        sim.update(dt.min(1.0 / 60.0));
        sim.draw(&context).unwrap();

        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}
