use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct ProgressTracker {
    total: usize,
    current: Arc<Mutex<usize>>,
    show_progress: bool,
}

impl ProgressTracker {
    pub fn new(total: usize, show_progress: bool) -> Self {
        let tracker = Self {
            total,
            current: Arc::new(Mutex::new(0)),
            show_progress,
        };

        if show_progress {
            tracker.spawn_progress_thread();
        }

        tracker
    }

    pub fn increment(&self) {
        let mut current = self.current.lock().unwrap();
        *current += 1;
    }

    pub fn finish(&self) {
        if self.show_progress {
            let current = *self.current.lock().unwrap();
            Self::progress_bar(current, self.total);
            println!();
        }
    }

    fn spawn_progress_thread(&self) {
        let current = Arc::clone(&self.current);
        let total = self.total;
        let show_progress = self.show_progress;

        thread::spawn(move || {
            let mut last_count = 0;
            while show_progress {
                thread::sleep(Duration::from_millis(200));
                let count = *current.lock().unwrap();
                if count < last_count {
                    Self::progress_bar(last_count, total);
                } else {
                    last_count = count;
                    Self::progress_bar(count, total);
                }
                if count >= total {
                    Self::progress_bar(total, total);
                    break;
                }
            }
        });
    }

    fn progress_bar(progress: usize, total: usize) {
        let width = 50;
        let percent = (progress * 100) / total;
        let filled = (width * progress) / total;
        let empty = width - filled;
    
        print!("\r[");
    
        if percent < 100 {
            if filled > 0 {
                for _ in 0..filled-1 {
                    print!("=");
                }
                print!(">");
            }
            for _ in 0..empty {
                print!(" ");
            }
        } else {
            for _ in 0..width {
                print!("=");
            }
        }
        print!("] {:3}% ({}/{})", percent, progress, total);
        std::io::stdout().flush().unwrap();
    }
}