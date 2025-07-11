use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use std::rc::Rc;
use std::cell::RefCell;
use std::f64::consts::PI;
use glam::DVec2;

fn random_dvec2() -> DVec2 {
    let angle = js_sys::Math::random() * 2.0 * PI;
    DVec2::new(angle.cos(), angle.sin())
}

#[derive(Clone, Copy, PartialEq)]
enum PheromoneType {
    ToHome,
    ToFood,
}

struct Pheromone {
    pos: DVec2,
    pheromone_type: PheromoneType,
    intensity: f64,
}

impl Pheromone {
    const EVAPORATION_TIME: f64 = 10.0;

    fn new(x: f64, y: f64, pheromone_type: PheromoneType) -> Self {
        Self {
            pos: DVec2::new(x, y),
            pheromone_type,
            intensity: 1.0,
        }
    }

    fn update(&mut self, dt: f64) {
        self.intensity -= dt / Self::EVAPORATION_TIME;
    }

    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        let alpha = self.intensity.max(0.0).min(1.0);
        let color = match self.pheromone_type {
            PheromoneType::ToHome => format!("rgba(255, 0, 0, {})", alpha),
            PheromoneType::ToFood => format!("rgba(0, 153, 153, {})", alpha),
        };
        ctx.set_fill_style(&JsValue::from_str(&color));
        ctx.begin_path();
        ctx.arc(self.pos.x, self.pos.y, 3.5, 0.0, 2.0 * PI).unwrap();
        ctx.fill();
    }
}

struct Cell {
    pos: DVec2,
    has_food: bool,
    to_home_pheromone: Option<Pheromone>,
    to_food_pheromone: Option<Pheromone>,
}

impl Cell {
    fn new(x: f64, y: f64) -> Self {
        Self {
            pos: DVec2::new(x, y),
            has_food: false,
            to_home_pheromone: None,
            to_food_pheromone: None,
        }
    }

    fn add_pheromone(&mut self, pheromone: Pheromone) {
        match pheromone.pheromone_type {
            PheromoneType::ToFood => {
                if self.to_food_pheromone.is_none() || self.to_food_pheromone.as_ref().unwrap().intensity < pheromone.intensity {
                    self.to_food_pheromone = Some(pheromone);
                }
            },
            PheromoneType::ToHome => {
                if self.to_home_pheromone.is_none() || self.to_home_pheromone.as_ref().unwrap().intensity < pheromone.intensity {
                    self.to_home_pheromone = Some(pheromone);
                }
            },
        }
    }

    fn update(&mut self, dt: f64) {
        if let Some(p) = &mut self.to_food_pheromone {
            p.update(dt);
            if p.intensity <= 0.0 { self.to_food_pheromone = None; }
        }
        if let Some(p) = &mut self.to_home_pheromone {
            p.update(dt);
            if p.intensity <= 0.0 { self.to_home_pheromone = None; }
        }
    }

    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        if let Some(p) = &self.to_food_pheromone { p.draw(ctx); }
        if let Some(p) = &self.to_home_pheromone { p.draw(ctx); }
        if self.has_food {
            ctx.set_fill_style(&JsValue::from_str("rgb(255, 204, 0)"));
            ctx.begin_path();
            ctx.arc(self.pos.x, self.pos.y, 3.5, 0.0, 2.0 * PI).unwrap();
            ctx.fill();
        }
    }
}

struct Grid {
    cells: Vec<Cell>,
    cols: i32,
    rows: i32,
    width: f64,
    height: f64,
}

impl Grid {
    fn new(width: f64, height: f64) -> Self {
        let cols = width.ceil() as i32 + 1;
        let rows = height.ceil() as i32 + 1;
        let mut cells = Vec::with_capacity((rows * cols) as usize);
        for i in 0..rows {
            for j in 0..cols {
                cells.push(Cell::new(j as f64 - width / 2.0, i as f64 - height / 2.0));
            }
        }
        Self { cells, cols, rows, width, height }
    }

    fn coords_to_index(&self, x: f64, y: f64) -> Option<usize> {
        let grid_x = (x + self.width / 2.0).round() as i32;
        let grid_y = (y + self.height / 2.0).round() as i32;
        if grid_x < 0 || grid_x >= self.cols || grid_y < 0 || grid_y >= self.rows {
            None
        } else {
            Some((grid_y * self.cols + grid_x) as usize)
        }
    }
    
    fn update(&mut self, dt: f64) {
        for cell in &mut self.cells { cell.update(dt); }
    }

    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        for cell in &self.cells { cell.draw(ctx); }
    }
}

struct Ant {
    pos: DVec2,
    vel: DVec2,
    target_vel: DVec2,
    pheromone_phase: PheromoneType,
    last_dir_change_time: f64,
    last_pheromone_drop_time: f64,
}

impl Ant {
    const MAX_SPEED: f64 = 80.0;
    const STEER_STRENGTH: f64 = 20.0;
    const DIR_CHANGE_TIME: f64 = 1.5;
    const PHEROMONE_DROP_TIME: f64 = 0.5;
    const BORDER: f64 = 100.0;
    const CHECK_RADIUS: f64 = 10.0;

    fn new(x: f64, y: f64) -> Self {
        let vel = random_dvec2() * Self::MAX_SPEED;
        Self {
            pos: DVec2::new(x, y),
            vel,
            target_vel: vel,
            pheromone_phase: PheromoneType::ToHome,
            last_dir_change_time: 0.0,
            last_pheromone_drop_time: 0.0,
        }
    }

    fn update(&mut self, dt: f64, grid: &mut Grid, home_pos: DVec2, width: f64, height: f64) {
        self.last_dir_change_time += dt;
        self.last_pheromone_drop_time += dt;

        if self.last_dir_change_time >= Self::DIR_CHANGE_TIME {
            self.change_dir();
        }

        if self.last_pheromone_drop_time >= Self::PHEROMONE_DROP_TIME {
            self.drop_pheromone(grid);
        }

        self.handle_wall_collision(width, height);

        match self.pheromone_phase {
            PheromoneType::ToHome => self.check_food(grid),
            PheromoneType::ToFood => self.check_home(home_pos),
        }

        self.update_position(dt, width, height);
    }
    
    fn update_position(&mut self, dt: f64, width: f64, height: f64) {
        let acc = (self.target_vel - self.vel).clamp_length_max(Self::STEER_STRENGTH);
        self.vel += acc * dt;
        self.vel = self.vel.clamp_length_max(Self::MAX_SPEED);
        self.pos += self.vel * dt;

        let half_w = width / 2.0;
        let half_h = height / 2.0;
        self.pos = self.pos.clamp(DVec2::new(-half_w, -half_h), DVec2::new(half_w, half_h));
    }
    
    fn change_dir(&mut self) {
        self.target_vel = random_dvec2() * Self::MAX_SPEED;
        self.last_dir_change_time = 0.0;
    }
    
    fn drop_pheromone(&mut self, grid: &mut Grid) {
        if let Some(index) = grid.coords_to_index(self.pos.x, self.pos.y) {
            let pheromone = Pheromone::new(self.pos.x, self.pos.y, self.pheromone_phase);
            grid.cells[index].add_pheromone(pheromone);
        }
        self.last_pheromone_drop_time = 0.0;
    }

    fn handle_wall_collision(&mut self, width: f64, height: f64) {
        let half_w = width / 2.0;
        let half_h = height / 2.0;
        if self.pos.x > half_w - Self::BORDER && self.target_vel.x > 0.0 {
            self.target_vel.x *= -1.0;
        } else if self.pos.x < -half_w + Self::BORDER && self.target_vel.x < 0.0 {
            self.target_vel.x *= -1.0;
        }
        if self.pos.y > half_h - Self::BORDER && self.target_vel.y > 0.0 {
            self.target_vel.y *= -1.0;
        } else if self.pos.y < -half_h + Self::BORDER && self.target_vel.y < 0.0 {
            self.target_vel.y *= -1.0;
        }
    }

    fn check_food(&mut self, grid: &mut Grid) {
        let r = Self::CHECK_RADIUS as i32;
        for i in -r..=r {
            for j in -r..=r {
                if (i * i + j * j) as f64 > Self::CHECK_RADIUS * Self::CHECK_RADIUS { continue; }
                
                let check_x = self.pos.x + i as f64;
                let check_y = self.pos.y + j as f64;

                if let Some(index) = grid.coords_to_index(check_x, check_y) {
                    if grid.cells[index].has_food {
                        grid.cells[index].has_food = false;
                        self.pheromone_phase = PheromoneType::ToFood;
                        self.vel *= -1.0;
                        self.target_vel = self.vel;
                        return;
                    }
                }
            }
        }
    }

    fn check_home(&mut self, home_pos: DVec2) {
        if self.pos.distance_squared(home_pos) <= Colony::RADIUS * Colony::RADIUS {
            self.pheromone_phase = PheromoneType::ToHome;
            self.vel *= -1.0;
            self.target_vel = self.vel;
        }
    }

    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        ctx.set_fill_style(&JsValue::from_str("black"));
        ctx.begin_path();
        ctx.arc(self.pos.x, self.pos.y, 3.0, 0.0, 2.0 * PI).unwrap();
        ctx.fill();

        ctx.set_stroke_style(&JsValue::from_str("white"));
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.move_to(self.pos.x, self.pos.y);
        let dir_end = self.pos + self.vel.normalize_or_zero() * 5.0;
        ctx.line_to(dir_end.x, dir_end.y);
        ctx.stroke();
    }
}

struct Colony {
    home: DVec2,
    ants: Vec<Ant>,
}

impl Colony {
    const RADIUS: f64 = 25.0;

    fn new(x: f64, y: f64, num_ants: u32) -> Self {
        let home = DVec2::new(x, y);
        let ants = (0..num_ants).map(|_| Ant::new(x, y)).collect();
        Self { home, ants }
    }

    fn update(&mut self, dt: f64, grid: &mut Grid, width: f64, height: f64) {
        for ant in &mut self.ants {
            ant.update(dt, grid, self.home, width, height);
        }
    }

    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        ctx.set_fill_style(&JsValue::from_str("blue"));
        ctx.begin_path();
        ctx.arc(self.home.x, self.home.y, Self::RADIUS, 0.0, 2.0 * PI).unwrap();
        ctx.fill();

        for ant in &self.ants {
            ant.draw(ctx);
        }
    }
}

struct Simulation {
    colony: Colony,
    grid: Grid,
    width: f64,
    height: f64,
}

impl Simulation {
    fn new(width: f64, height: f64) -> Self {
        let mut grid = Grid::new(width, height);
        let colony = Colony::new(0.0, 0.0, 100);

        let food_x = -200.0;
        let food_y = -200.0;
        let food_size = 25;
        for i in -food_size..=food_size {
            for j in -food_size..=food_size {
                if let Some(index) = grid.coords_to_index(food_x + i as f64, food_y + j as f64) {
                    grid.cells[index].has_food = true;
                }
            }
        }
        
        Self { colony, grid, width, height }
    }

    fn update(&mut self, dt: f64) {
        self.colony.update(dt, &mut self.grid, self.width, self.height);
        self.grid.update(dt);
    }

    fn draw(&self, ctx: &CanvasRenderingContext2d) {
        ctx.set_fill_style(&JsValue::from_str("#DDDDDD"));
        ctx.fill_rect(-self.width/2.0, -self.height/2.0, self.width, self.height);

        self.grid.draw(ctx);
        self.colony.draw(ctx);
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

    let simulation = Rc::new(RefCell::new(Simulation::new(width, height)));
    
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    
    let mut last_time = window.performance().unwrap().now();
    
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let now = web_sys::window().unwrap().performance().unwrap().now();
        let dt = (now - last_time) / 1000.0;
        last_time = now;

        let mut sim = simulation.borrow_mut();
        sim.update(dt.min(1.0/30.0));
        sim.draw(&context);

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
