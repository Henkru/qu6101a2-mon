mod app;
mod constants;
mod data;
mod input;
mod transport;
mod ui;

#[cfg(debug_assertions)]
mod sim;

use std::io;
use std::sync::mpsc;
use std::time::Duration;

use clap::Parser;
use color_eyre::eyre::{self, WrapErr};
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::AppState;
use input::handle_key_event;
use transport::{spawn_worker, TransportConfig, TransportEvent};
use ui::render_ui;

#[cfg(debug_assertions)]
#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Quick 6101A2 TUI monitor")]
struct Args {
    /// Serial port path (e.g. /dev/ttyUSB0)
    #[arg(short, long, required_unless_present = "simulate")]
    port: Option<String>,

    /// Serial baud rate
    #[arg(short, long, default_value_t = 19_200)]
    baud: u32,

    /// Modbus device address
    #[arg(short, long, default_value_t = 2)]
    address: u8,

    /// Poll interval in milliseconds
    #[arg(short = 'i', long, default_value_t = 500)]
    poll_interval: u64,

    /// Run without a serial device
    #[arg(short = 's', long, default_value_t = false)]
    simulate: bool,

    /// Disable write commands
    #[arg(short = 'r', long, default_value_t = false)]
    read_only: bool,
}

#[cfg(not(debug_assertions))]
#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Quick 6101A2 TUI monitor")]
struct Args {
    /// Serial port path (e.g. /dev/ttyUSB0)
    #[arg(short, long)]
    port: String,

    /// Serial baud rate
    #[arg(short, long, default_value_t = 19_200)]
    baud: u32,

    /// Modbus device address
    #[arg(short, long, default_value_t = 2)]
    address: u8,

    /// Poll interval in milliseconds
    #[arg(short = 'i', long, default_value_t = 500)]
    poll_interval: u64,

    /// Disable write commands
    #[arg(short = 'r', long, default_value_t = false)]
    read_only: bool,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    enable_raw_mode().wrap_err("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).wrap_err("enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    let config = TransportConfig {
        port: port_value(&args),
        baud: args.baud,
        address: args.address,
        poll_interval: Duration::from_millis(args.poll_interval),
        read_only: args.read_only,
        simulate: simulate_enabled(&args),
    };

    let serial_handle = spawn_worker(config, command_rx, event_tx);

    let tick_rate = Duration::from_millis(100);
    let mut app = AppState::new(simulate_enabled(&args), args.read_only);
    let mut exit_error: Option<eyre::Report> = None;

    loop {
        terminal.draw(|frame| render_ui(frame, &app))?;

        if event::poll(tick_rate)?
            && let Event::Key(key) = event::read()?
            && handle_key_event(key.code, &mut app, &command_tx)?
        {
            break;
        }

        match event_rx.try_recv() {
            Ok(TransportEvent::Status(status)) => app.update_status(status),
            Ok(TransportEvent::Error(err)) => {
                exit_error = Some(err.wrap_err("serial connection failed"));
                app.should_quit = true;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                exit_error = Some(eyre::eyre!("serial thread disconnected"));
                app.should_quit = true;
            }
        }

        if app.should_quit {
            break;
        }
    }

    command_tx.send(transport::TransportCommand::Terminate).ok();
    serial_handle.join().ok();

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    if let Some(err) = exit_error {
        return Err(err);
    }

    Ok(())
}

#[cfg(debug_assertions)]
fn port_value(args: &Args) -> Option<String> {
    args.port.clone()
}

#[cfg(not(debug_assertions))]
fn port_value(args: &Args) -> Option<String> {
    Some(args.port.clone())
}

#[cfg(debug_assertions)]
fn simulate_enabled(args: &Args) -> bool {
    args.simulate
}

#[cfg(not(debug_assertions))]
fn simulate_enabled(_args: &Args) -> bool {
    false
}
