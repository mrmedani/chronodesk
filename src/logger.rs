use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

static LOG: Mutex<Option<PathBuf>> = Mutex::new(None);

fn log_path() -> PathBuf {
    let path = if cfg!(target_os = "windows") {
        std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string()) + "\\chronodesk"
    } else {
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string()) + "/.chronodesk"
    };
    PathBuf::from(&path).join("chronodesk.log")
}

pub fn init() {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, "");
    let mut log = LOG.lock().unwrap_or_else(|e| e.into_inner());
    *log = Some(path);

    let panic_path = log_path().clone();
    let desk = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").map(|p| format!("{p}\\Desktop"))
    } else {
        std::env::var("HOME").map(|p| format!("{p}/Desktop"))
    }
    .unwrap_or_default();

    std::panic::set_hook(Box::new(move |info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        let location = info.location().map(|l| l.to_string()).unwrap_or_default();
        let full = format!("PANIC: {msg} at {location}");
        write_log(&full);
        // Copy log to Desktop immediately before crash
        if let Ok(log) = std::fs::read_to_string(&panic_path) {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let dest = format!("{desk}\\chronodesk_crash_{ts}.log");
            let _ = std::fs::write(&dest, &log);
            let _ = std::fs::write(dest.replace(".log", "_panic.txt"), &full);
        }
    }));
}

pub fn write_log(msg: &str) {
    let path = {
        let log = LOG.lock().unwrap_or_else(|e| e.into_inner());
        log.clone()
    };
    let path = match path {
        Some(p) => p,
        None => {
            let p = log_path();
            let _ = std::fs::write(&p, "");
            p
        }
    };
    let ts = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let line = format!("[{ts}] {msg}\n");
    if let Ok(mut f) = OpenOptions::new().append(true).create(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}

pub fn read_log() -> String {
    let path = {
        let log = LOG.lock().unwrap_or_else(|e| e.into_inner());
        log.clone()
    };
    match path {
        Some(p) => std::fs::read_to_string(&p).unwrap_or_default(),
        None => String::new(),
    }
}
