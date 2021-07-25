extern crate chrono;

use chrono::offset::Utc;
use chrono::DateTime;
use std::time::SystemTime;

use std::ptr;
use std::ffi::CString;
use std::mem;
use std::env;
use std::fs::{File, OpenOptions};
use std::time::{ UNIX_EPOCH, Duration};
use std:: thread;
use std::sync::{Arc, Mutex};
use x11::xinput2::*;
use x11::xlib::{XOpenDisplay, XDefaultScreenOfDisplay, XRootWindowOfScreen, Display,
                Window, XFlush, XEvent, XNextEvent, XQueryExtension};
use std::io::Write;

const DEFAULT_OUT_FILE : &str = "/tmp/X11_WATCHER.txt";

pub struct Config {
    outfile: File,
    idletime: Duration,
    thread_sleep: Duration
}

impl Config {
    fn log_to_file(&mut self, msg: &String) {
        self.outfile.write_all(msg.as_bytes()).unwrap();
    }

    fn new() -> Config {
        let default_file_name = String::from(DEFAULT_OUT_FILE);
        let default_idle = String::from("5");
        let default_sleep = String::from("30");

        let args: Vec<String> = env::args().collect();
        let file = args.get(1).unwrap_or(&default_file_name);
        let idle_minutes = args.get(2).unwrap_or(&default_idle);
        let thread_sleep_secs = args.get(3).unwrap_or(&default_sleep);

        let outfile = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(file)
            .expect("could not open log file");

        let idletime = Duration::from_secs(idle_minutes.parse::<u64>().unwrap() * 60);
        let thread_sleep = Duration::from_secs(thread_sleep_secs.parse::<u64>().unwrap() * 1);

        Config {
            outfile,
            idletime,
            thread_sleep
        }
    }
}

pub struct WindowSystem {
    display: *mut Display,
    root:    Window
}

impl WindowSystem {
    pub fn new() -> WindowSystem {
        unsafe {
            let display = XOpenDisplay(ptr::null_mut());
            let screen = XDefaultScreenOfDisplay(display);
            let root = XRootWindowOfScreen(screen);

            WindowSystem { display, root }
        }
    }

    unsafe fn check_extension(&self) {
        let opcode: *mut i32 = &mut 0;
        let event: *mut i32 = &mut 0;
        let error: *mut i32 = &mut 0;

        let result = XQueryExtension(
            self.display,
            CString::new("XInputExtension").unwrap().as_ptr(),
            opcode,
            event,
            error);

        if result != 1 {
            panic!("missing XInputExtension");
        }
    }
}

macro_rules! now {
    () => {{
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("get system time")
            .as_millis()
        }}        
}

fn main() {
    let last_event_ts = Arc::new(Mutex::new(now!()));
    let last_event_ts_clone = last_event_ts.clone();

    thread::spawn(move || {
        let mut last_matched_ts : u128 = 0;
        let mut config = Config::new();
        let ideltime = config.idletime.as_millis();

        loop {
            thread::sleep(config.thread_sleep);
            let now = now!();
            let local_last_ts = *last_event_ts.lock().unwrap();

            if last_matched_ts != local_last_ts && now - local_last_ts > ideltime {
                last_matched_ts = local_last_ts;

                let datetime: DateTime<Utc> = SystemTime::now().into();
                let datetime = datetime.format("%d/%m/%Y %T");
                let msg = format!("idle threshold reached: {}\n", datetime);
                config.log_to_file(&msg);
            }
        }
    });

    unsafe {
        let window_system = WindowSystem::new();
        window_system.check_extension();

        let mut mask: [::std::os::raw::c_uchar; 1] = mem::zeroed();
        XISetMask(&mut mask, XI_ButtonPress);
        XISetMask(&mut mask, XI_KeyPress);
        XISetMask(&mut mask, XI_Motion);

        let mut ev_mask = XIEventMask {
            deviceid: XIAllDevices,
            mask: mask.as_mut_ptr(),
            mask_len: 1
        };

        let ev_mask_ptr: *mut XIEventMask = &mut ev_mask;

        XISelectEvents(window_system.display, window_system.root, ev_mask_ptr, 1);
        XFlush(window_system.display);

        loop {
            let mut event: XEvent = mem::zeroed();
            XNextEvent(window_system.display, &mut event);

            let now = now!();
            *last_event_ts_clone.lock().unwrap() = now;
        }
    }
}
