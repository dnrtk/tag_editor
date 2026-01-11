use std::path::PathBuf;
use std::time::Instant;

pub struct Slideshow {
    /// スライドショーが実行中かどうか
    pub is_running: bool,
    /// スライドショー対象の画像リスト
    pub images: Vec<PathBuf>,
    /// 現在のインデックス
    pub current_index: usize,
    /// 最後に画像を切り替えた時刻
    last_switch: Instant,
    /// 1巡目が完了したか
    pub completed_once: bool,
}

impl Default for Slideshow {
    fn default() -> Self {
        Self {
            is_running: false,
            images: Vec::new(),
            current_index: 0,
            last_switch: Instant::now(),
            completed_once: false,
        }
    }
}

impl Slideshow {
    /// スライドショーを開始
    pub fn start(&mut self, images: Vec<PathBuf>) {
        self.images = images;
        self.current_index = 0;
        self.is_running = !self.images.is_empty();
        self.last_switch = Instant::now();
        self.completed_once = false;
    }

    /// スライドショーを停止
    pub fn stop(&mut self) {
        self.is_running = false;
    }

    /// 更新処理（intervalは秒単位、loopは繰り返すかどうか）
    /// 次の画像のパスを返す場合がある
    pub fn update(&mut self, interval: f32, should_loop: bool) -> Option<PathBuf> {
        if !self.is_running || self.images.is_empty() {
            return None;
        }

        let elapsed = self.last_switch.elapsed().as_secs_f32();
        if elapsed >= interval {
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

            return self.images.get(self.current_index).cloned();
        }

        None
    }

    /// 現在の画像パスを取得
    pub fn current_image(&self) -> Option<&PathBuf> {
        self.images.get(self.current_index)
    }
}
