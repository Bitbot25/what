use chrono;
use std::time;
use std::io::{self, Read};
use std::fmt;
use std::fs;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::cell::RefCell;

trait Module<W: io::Write> {
    fn ready(&mut self) -> bool;
    fn go(&mut self, ready: bool, buf: &mut W) -> io::Result<()>;
}

struct Time {
    l_act: time::SystemTime,
    cache: Option<String>,
}

impl<W: io::Write> Module<W> for Time {
    fn ready(&mut self) -> bool {
        self.l_act.elapsed().unwrap() >= time::Duration::from_secs(1)
    }

    fn go(&mut self, ready: bool, buf: &mut W) -> io::Result<()> {
        if ready {
            self.l_act = time::SystemTime::now();
            self.cache = Some(chrono::Local::now().format("%H:%M:%S").to_string());
        }
        write!(buf, "{}", self.cache.as_ref().unwrap())
    }
}

impl Default for Time {
    fn default() -> Time {
        Time { l_act: time::SystemTime::UNIX_EPOCH, cache: None }
    }
}

const BATTERY_PATH: &'static str = "/sys/class/power_supply/BAT0/capacity";
struct BatteryLevel {
    current: String,
    last_hash: u64,
}

fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

impl<W: io::Write> Module<W> for BatteryLevel {
    fn ready(&mut self) -> bool {
        self.current = fs::read_to_string(BATTERY_PATH).unwrap();
        let changed = hash_str(&*self.current) != self.last_hash;

        if changed {
            self.last_hash = hash_str(&*self.current);
        }

        changed
    }

    fn go(&mut self, ready: bool, buf: &mut W) -> io::Result<()> {
        if self.last_hash == 0 {
            // Initialize
            self.current = fs::read_to_string(BATTERY_PATH).unwrap();
            self.last_hash = hash_str(&*self.current);
        }

        write!(buf, "{}% Battery", &self.current[0..self.current.len() - 1])
    }
}

impl Default for BatteryLevel {
    fn default() -> BatteryLevel {
        BatteryLevel { current: String::new(), last_hash: 0 }
    }
}

fn modules_go<const N: usize, W: io::Write>(modules: &mut [(&mut dyn Module<W>, bool); N], output: &mut W) -> io::Result<()> {
    let mut iter = modules.iter_mut();
    if let Some((m, is_ready)) = iter.next() {
        m.go(*is_ready, output)?;
    } else {
        return Ok(());
    }

    for (m, is_ready) in iter {
        print!(" | ");
        m.go(*is_ready, output)?;
    }
    print!("\n");
    Ok(())
}

fn main() {
    const ACCEPTABLE_SLEEP: time::Duration = time::Duration::from_millis(100);
    // /sys/class/power_supply/BAT0/capacity
    let mut time = Time::default();
    let mut battery = BatteryLevel::default();

    const MODULES_LEN: usize = 2;
    let mut modules: [(&mut dyn Module<io::Stdout>, bool); MODULES_LEN] = [(&mut time, false), (&mut battery, false)];
    loop {
        let begin = time::SystemTime::now();
        let mut updated = false;
        for (i, (m, ready)) in modules.iter_mut().enumerate() {
            *ready = m.ready();
            if *ready {
                updated = true;
            } 
        }

        if updated {
            modules_go(&mut modules, &mut io::stdout()).unwrap();
        }

        std::thread::yield_now();
        std::hint::spin_loop();
        let duration = begin.elapsed().unwrap();
        if duration < ACCEPTABLE_SLEEP {
            // Sleep until each loop iteration takes ACCEPTABLE_SLEEP time.
            std::thread::sleep(ACCEPTABLE_SLEEP - duration);
        }
    }
}
