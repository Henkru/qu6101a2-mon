mod app;
mod backend;
mod constants;
mod data;
mod interface;
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
use interface::InterfaceMode;
use input::handle_key_event;
use transport::{spawn_worker, TransportConfig, TransportEvent};
use ui::render_ui;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Quick 6101A2 TUI monitor")]
struct Args {
    /// Serial port path (e.g. /dev/ttyUSB0)
    #[arg(short, long)]
    port: Option<String>,

    /// Serial baud rate
    #[arg(short, long)]
    baud: Option<u32>,

    /// Modbus device address
    #[arg(short, long)]
    address: Option<u8>,

    /// Poll interval in milliseconds
    #[arg(short = 'i', long, default_value_t = 500)]
    poll_interval: u64,

    /// Device interface
    #[arg(short = 'I', long, value_enum, default_value_t = InterfaceMode::Remote)]
    interface: InterfaceMode,

    /// Disable write commands
    #[arg(short = 'r', long, default_value_t = false)]
    read_only: bool,
}

#[derive(Debug, Clone)]
struct RuntimeArgs {
    transport: TransportConfig,
    read_only: bool,
    simulate_ui: bool,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let runtime = resolve_runtime_args(&args)?;

    enable_raw_mode().wrap_err("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).wrap_err("enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    let serial_handle = spawn_worker(runtime.transport.clone(), command_rx, event_tx);

    let tick_rate = Duration::from_millis(100);
    let mut app = AppState::new(runtime.simulate_ui, runtime.read_only);
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
            Ok(TransportEvent::Connection(connected)) => app.connected = connected,
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

fn resolve_runtime_args(args: &Args) -> eyre::Result<RuntimeArgs> {
    let interface = {
        #[cfg(debug_assertions)]
        {
            resolve_interface_mode(args)
        }
        #[cfg(not(debug_assertions))]
        {
            resolve_interface_mode(args)?
        }
    };
    let baud = args.baud.unwrap_or(interface.default_baud());
    let address = args.address.unwrap_or(interface.default_address());

    let port = match interface {
        InterfaceMode::Simulation => None,
        _ => Some(
            args.port
                .clone()
                .ok_or_else(|| eyre::eyre!("serial port required unless using simulation interface"))?,
        ),
    };

    Ok(RuntimeArgs {
        transport: TransportConfig {
            port,
            baud,
            address,
            poll_interval: Duration::from_millis(args.poll_interval),
            read_only: args.read_only,
            interface,
        },
        read_only: args.read_only,
        simulate_ui: interface == InterfaceMode::Simulation,
    })
}

#[cfg(debug_assertions)]
fn resolve_interface_mode(args: &Args) -> InterfaceMode {
    args.interface
}

#[cfg(not(debug_assertions))]
fn resolve_interface_mode(args: &Args) -> eyre::Result<InterfaceMode> {
    if args.interface == InterfaceMode::Simulation {
        return Err(eyre::eyre!(
            "simulation interface is only available in debug builds"
        ));
    }
    Ok(args.interface)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Args, InterfaceMode, resolve_runtime_args};

    #[test]
    fn remote_defaults_match_existing_behavior() {
        let args = Args::try_parse_from(["bin", "--port", "/dev/ttyUSB0"])
            .expect("args should parse");
        let runtime = resolve_runtime_args(&args).expect("runtime should resolve");
        assert_eq!(runtime.transport.interface, InterfaceMode::Remote);
        assert_eq!(runtime.transport.baud, 19_200);
        assert_eq!(runtime.transport.address, 2);
    }

    #[test]
    fn exttool_defaults_are_selected_from_interface() {
        let args = Args::try_parse_from([
            "bin",
            "--port",
            "/dev/ttyUSB0",
            "--interface",
            "exttool",
        ])
        .expect("args should parse");
        let runtime = resolve_runtime_args(&args).expect("runtime should resolve");
        assert_eq!(runtime.transport.interface, InterfaceMode::Exttool);
        assert_eq!(runtime.transport.baud, 38_400);
        assert_eq!(runtime.transport.address, 1);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn simulation_interface_works_without_port() {
        let args =
            Args::try_parse_from(["bin", "--interface", "simulation"]).expect("args should parse");
        let runtime = resolve_runtime_args(&args).expect("runtime should resolve");
        assert_eq!(runtime.transport.interface, InterfaceMode::Simulation);
        assert!(runtime.transport.port.is_none());
    }

    #[test]
    fn explicit_baud_and_address_override_interface_defaults() {
        let args = Args::try_parse_from([
            "bin",
            "--port",
            "/dev/ttyUSB0",
            "--interface",
            "exttool",
            "--baud",
            "57600",
            "--address",
            "7",
        ])
        .expect("args should parse");
        let runtime = resolve_runtime_args(&args).expect("runtime should resolve");
        assert_eq!(runtime.transport.baud, 57_600);
        assert_eq!(runtime.transport.address, 7);
    }

    #[test]
    fn serial_interfaces_require_port() {
        let args = Args::try_parse_from(["bin"]).expect("args should parse");
        let err = resolve_runtime_args(&args).expect_err("port should be required");
        assert!(err.to_string().contains("serial port required"));
    }
}
