use std::sync::mpsc::Sender;

use color_eyre::eyre::{self, WrapErr};
use crossterm::event::KeyCode;

use crate::app::AppState;
use crate::constants::{STATE_ON, TARGET_FLOW_MAX, TARGET_FLOW_MIN};
use crate::transport::TransportCommand;

pub fn handle_key_event(
    code: KeyCode,
    app: &mut AppState,
    command_tx: &Sender<TransportCommand>,
) -> eyre::Result<bool> {
    if app.input_mode {
        handle_input_event(code, app, command_tx)?;
        return Ok(false);
    }

    match code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            return Ok(true);
        }
        KeyCode::Char(' ') => {
            if app.read_only {
                return Ok(false);
            }
            if let Some(status) = &app.status {
                let next_state = status.state != STATE_ON;
                command_tx
                    .send(TransportCommand::SetPower(next_state))
                    .wrap_err("send power toggle")?;
            }
        }
        KeyCode::Left => {
            if app.read_only {
                return Ok(false);
            }
            if app.target_flow > TARGET_FLOW_MIN {
                app.target_flow -= 1;
                send_target_flow(command_tx, app.target_flow)?;
            }
        }
        KeyCode::Right => {
            if app.read_only {
                return Ok(false);
            }
            if app.target_flow < TARGET_FLOW_MAX {
                app.target_flow += 1;
                send_target_flow(command_tx, app.target_flow)?;
            }
        }
        KeyCode::Char('d') => {
            app.show_debug = !app.show_debug;
        }
        KeyCode::Char('t') => {
            if !app.read_only {
                app.input_mode = true;
                app.input_buffer.clear();
            }
        }
        _ => {}
    }

    Ok(false)
}

fn handle_input_event(
    code: KeyCode,
    app: &mut AppState,
    command_tx: &Sender<TransportCommand>,
) -> eyre::Result<()> {
    match code {
        KeyCode::Esc => {
            app.input_mode = false;
            app.input_buffer.clear();
        }
        KeyCode::Enter => {
            if let Ok(value) = app.input_buffer.parse::<u16>() {
                let clamped = value.clamp(TARGET_FLOW_MIN, TARGET_FLOW_MAX);
                app.target_flow = clamped;
                send_target_flow(command_tx, clamped)?;
            }
            app.input_mode = false;
            app.input_buffer.clear();
        }
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(ch) if ch.is_ascii_digit() => {
            if app.input_buffer.len() < 3 {
                app.input_buffer.push(ch);
            }
        }
        _ => {}
    }
    Ok(())
}

fn send_target_flow(command_tx: &Sender<TransportCommand>, value: u16) -> eyre::Result<()> {
    command_tx
        .send(TransportCommand::SetTargetFlow(value))
        .wrap_err("send target flow")
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use crossterm::event::KeyCode;

    use crate::app::AppState;
    use crate::constants::{STATE_OFF, STATE_ON};
    use crate::data::DeviceStatus;
    use crate::input::handle_key_event;
    use crate::interface::InterfaceMode;
    use crate::transport::TransportCommand;

    #[test]
    fn read_only_mode_does_not_emit_write_commands() {
        let (tx, rx) = mpsc::channel();
        let mut app = AppState::new(InterfaceMode::Remote, true);
        app.status = Some(sample_status(STATE_OFF));

        handle_key_event(KeyCode::Char(' '), &mut app, &tx).expect("space key should work");
        handle_key_event(KeyCode::Left, &mut app, &tx).expect("left key should work");
        handle_key_event(KeyCode::Right, &mut app, &tx).expect("right key should work");

        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn power_toggle_sends_expected_command() {
        let (tx, rx) = mpsc::channel();
        let mut app = AppState::new(InterfaceMode::Remote, false);
        app.status = Some(sample_status(STATE_ON));

        handle_key_event(KeyCode::Char(' '), &mut app, &tx).expect("space key should work");

        assert_eq!(
            rx.recv().expect("command expected"),
            TransportCommand::SetPower(false)
        );
    }

    #[test]
    fn typed_target_flow_is_clamped_before_send() {
        let (tx, rx) = mpsc::channel();
        let mut app = AppState::new(InterfaceMode::Remote, false);
        app.input_mode = true;
        app.input_buffer = String::from("999");

        handle_key_event(KeyCode::Enter, &mut app, &tx).expect("enter key should work");

        assert_eq!(
            rx.recv().expect("command expected"),
            TransportCommand::SetTargetFlow(100)
        );
    }

    fn sample_status(state: u16) -> DeviceStatus {
        DeviceStatus {
            state,
            target_flow: 60,
            real_flow: 55,
            speed_rpm: 2000,
            p_filter_total: 50,
            m_filter_total: 300,
            c_filter_total: 600,
            p_filter_limit: 200,
            m_filter_limit: 1200,
            c_filter_limit: 2400,
            registers: vec![0; 24],
        }
    }
}
