use rgbmon::{OpenRGBClient, RGBColor, VERSION};

#[macro_use]
extern crate lazy_static;

use chrono::prelude::*;
use clap::Clap;
use colored::Colorize;
use cpu_monitor::CpuInstant;
use daemonize::Daemonize;
use signal_hook::{
    consts::{SIGHUP, SIGINT, SIGTERM, SIGUSR1},
    iterator::Signals,
};
use std::io::Write;
use std::process;
use std::sync::RwLock;
use std::thread;
use std::time::Duration;

use log::LevelFilter;
use log::{debug, error, info, Level, Metadata, Record};
use syslog::{BasicLogger, Facility, Formatter3164};

const COLORS: u32 = 0xFFFFFF;
const START: u32 = 4340064;
const END: u32 = 0xFFFFFF;

#[derive(Clap)]
#[clap(version = VERSION, about = "https://github.com/divi255/rgbmon")]
struct Opts {
    #[clap(short = 'v', long = "verbose", about = "Verbose output")]
    verbose: bool,
    #[clap(short = 'D', about = "Run in background")]
    daemonize: bool,
    #[clap(
        short = 's',
        long = "sleep step",
        about = "Sleep step",
        default_value = "1"
    )]
    sleep_step: f32,
    #[clap(
        short = 'x',
        long = "load diff",
        about = "Load diff",
        default_value = "1"
    )]
    load_diff: u8,
    #[clap(
        long = "default-color",
        about = "Default color for low CPU load (N:RRGGBB)"
    )]
    default_color: Option<String>,
    #[clap(
        long = "pid-file",
        about = "Pid file location",
        default_value = "/var/run/rgbmon.pid"
    )]
    pid_file: String,
    #[clap(
        long = "connect",
        about = "OpenRGB server host:port to connect to",
        default_value = "127.0.0.1:6742"
    )]
    connect: String,
    #[clap(
        long = "device-types",
        about = "Device types to operate, comma separated",
        default_value = "0,1,2,3,4",
        multiple = true,
        value_delimiter = ","
    )]
    device_types: Vec<u32>,
}

struct State {
    load: u8,
    color: RGBColor,
    min_load: Option<u8>,
    default_color: Option<RGBColor>,
    active: bool,
    device_types: Vec<u32>,
}

impl State {
    fn new() -> Self {
        Self {
            load: std::u8::MAX,
            color: RGBColor::new(0, 0, 0),
            min_load: None,
            default_color: None,
            active: true,
            device_types: Vec::new(),
        }
    }

    fn stop(&mut self) {
        self.active = false;
        debug!("Suspending");
        let _ = ORGB
            .write()
            .unwrap()
            .set_color_by_device_types(&self.device_types, &RGBColor::black())
            .map_err(|e| error!("Unable to set color: {}", e));
    }

    fn start(&mut self) {
        debug!("Resuming");
        self.active = true;
        self.apply(true);
    }

    fn apply(&mut self, force: bool) {
        if self.active && self.load != std::u8::MAX {
            let color;
            if self.min_load.is_some() && self.load as u8 <= self.min_load.unwrap() {
                color = self.default_color.unwrap().clone();
            } else {
                color = RGBColor::rainbow(self.load as u32, COLORS, START, END);
            }
            if force || color != self.color {
                debug!("Setting color: {}", color.colorize_self());
                match ORGB
                    .write()
                    .unwrap()
                    .set_color_by_device_types(&self.device_types, &color)
                {
                    Ok(_) => self.color = color,
                    Err(e) => {
                        error!("Unable to set color: {}", e);
                    }
                }
            }
        }
    }

    fn set_load(&mut self, load: u8) {
        self.load = load;
        self.apply(false);
    }
}

lazy_static! {
    static ref STATE: RwLock<State> = RwLock::new(State::new());
    static ref ORGB: RwLock<OpenRGBClient> = RwLock::new(OpenRGBClient::new());
}

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let s = format!(
                "{}  {}",
                Local::now().to_rfc3339_opts(SecondsFormat::Secs, false),
                record.args()
            );
            println!(
                "{}",
                match record.level() {
                    Level::Debug => s.dimmed(),
                    Level::Warn => s.yellow().bold(),
                    Level::Error => s.red(),
                    _ => s.normal(),
                }
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

fn set_verbose_logger(filter: LevelFilter) {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(filter))
        .unwrap();
}

fn main() {
    #[cfg(windows)]
    colored::control::set_override(false);
    let mut opts: Opts = Opts::parse();
    if opts.verbose {
        set_verbose_logger(LevelFilter::Debug);
    } else if std::env::var("DISABLE_SYSLOG").unwrap_or("0".to_owned()) == "1" {
        set_verbose_logger(LevelFilter::Info);
    } else {
        let formatter = Formatter3164 {
            facility: Facility::LOG_USER,
            hostname: None,
            process: "rgbmon".into(),
            pid: 0,
        };
        match syslog::unix(formatter) {
            Ok(logger) => {
                log::set_boxed_logger(Box::new(BasicLogger::new(logger)))
                    .map(|()| log::set_max_level(LevelFilter::Info))
                    .unwrap();
            }
            Err(_) => {
                set_verbose_logger(LevelFilter::Info);
            }
        }
    }
    debug!(
        "Device types managed: {}",
        opts.device_types
            .clone()
            .into_iter()
            .map(|i| i.to_string() + " ")
            .collect::<String>()
    );
    {
        let mut client = ORGB.write().unwrap();
        let mut state = STATE.write().unwrap();
        client.set_path(&opts.connect);
        state.device_types.append(&mut opts.device_types);
        match client.load() {
            Ok(_) => {
                if client.controllers.is_empty() {
                    error!("no controllers connected");
                } else {
                    let mut found = false;
                    for c in &client.controllers {
                        if state.device_types.contains(&c.device_type) {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        error!("no device types to control");
                    }
                }
            }
            Err(e) => error!("Server connection error: {}", e),
        }
    }
    let sleep_step: Duration = Duration::from_millis((opts.sleep_step * 1000.) as u64);
    let mut signals = Signals::new(&[SIGHUP, SIGUSR1, SIGINT, SIGTERM]).unwrap();
    let pid_file = opts.pid_file;
    debug!("Writing pid file: {}", pid_file);
    if opts.daemonize {
        Daemonize::new().pid_file(&pid_file).start().unwrap();
    } else {
        std::fs::File::create(&pid_file)
            .map_err(|e| {
                println!(
                    "{}",
                    format!("Unable to create pid file {}: {}", &pid_file, e).red()
                )
            })
            .unwrap()
            .write_all(format!("{}", process::id()).as_bytes())
            .unwrap();
    }
    thread::spawn(move || {
        for sig in signals.forever() {
            debug!("Received signal {:?}", sig);
            match sig {
                SIGHUP => {
                    info!("Reloading data");
                    let _ = ORGB.write().unwrap().reload();
                    STATE.write().unwrap().start();
                }
                SIGUSR1 => STATE.write().unwrap().stop(),
                SIGTERM | SIGINT => {
                    let _ = std::fs::remove_file(pid_file);
                    process::exit(0);
                }
                _ => {}
            }
        }
    });
    match opts.default_color {
        Some(s) => {
            let mut state = STATE.write().unwrap();
            let v: Vec<&str> = s.split(':').collect();
            state.min_load = Some(v[0].parse().unwrap());
            let c = RGBColor::from_str(v[1]);
            debug!(
                "Default color for load < {}: {}",
                state.min_load.unwrap(),
                c.colorize_self(),
            );
            state.default_color = Some(c);
        }
        None => {}
    }
    info!("started");
    loop {
        let start = CpuInstant::now().unwrap();
        thread::sleep(sleep_step);
        let end = CpuInstant::now().unwrap();
        let mut load = ((end - start).non_idle() * 100.) as u8;
        debug!("CPU load: {}", format!("{}%", &load).cyan());
        if load < opts.load_diff {
            load = 0;
        }
        let prev_load = STATE.read().unwrap().load;
        if prev_load == std::u8::MAX
            || (prev_load as i16 - load as i16).abs() as u8 >= opts.load_diff
        {
            STATE.write().unwrap().set_load(load);
        }
    }
}
