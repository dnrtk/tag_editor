use std::path::PathBuf;
use std::time::Instant;

pub struct Slideshow {
    pub is_running: bool,
    pub images: Vec<PathBuf>,
    pub current_index: usize,
    pub completed_once: bool,
    last_switch: Instant,
}

impl Default for Slideshow {
    fn default() -> Self {
        Self {
            is_running: false,
            images: Vec::new(),
            current_index: 0,
            completed_once: false,
            last_switch: Instant::now(),
        }
    }
}

impl Slideshow {
    pub fn start(&mut self, images: Vec<PathBuf>) {
        self.is_running = !images.is_empty();
        self.images = images;
        self.current_index = 0;
        self.completed_once = false;
        self.last_switch = Instant::now();
    }

    pub fn stop(&mut self) {
        self.is_running = false;
    }

    /// Advances to the next image when `interval` seconds have elapsed.
    /// Returns the new image path if the slideshow advanced, `None` otherwise.
    pub fn update(&mut self, interval_secs: f32, should_loop: bool) -> Option<PathBuf> {
        if !self.is_running || self.images.is_empty() {
            return None;
        }
        if self.last_switch.elapsed().as_secs_f32() < interval_secs {
            return None;
        }

        self.last_switch = Instant::now();
        self.current_index += 1;

        if self.current_index >= self.images.len() {
            self.completed_once = true;
            if should_loop {
                self.current_index = 0;
            } else {
                self.is_running = false;
                return None;
            }
        }
        Some(self.images[self.current_index].clone())
    }

    pub fn current_image(&self) -> Option<&PathBuf> {
        self.images.get(self.current_index)
    }
}
