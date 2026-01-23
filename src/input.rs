use std::sync::mpsc::Sender;

use color_eyre::eyre::{self, WrapErr};
use crossterm::event::KeyCode;

use crate::app::AppState;
use crate::constants::{
    REG_STATE, REG_TARGET_FLOW, STATE_OFF, STATE_ON, TARGET_FLOW_MAX, TARGET_FLOW_MIN,
};
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
                let next_state = if status.state == STATE_ON {
                    STATE_OFF
                } else {
                    STATE_ON
                };
                command_tx
                    .send(TransportCommand::WriteRegister {
                        register: REG_STATE,
                        value: next_state,
                    })
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
        .send(TransportCommand::WriteRegister {
            register: REG_TARGET_FLOW,
            value,
        })
        .wrap_err("send target flow")
}
