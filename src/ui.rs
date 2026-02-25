use std::collections::VecDeque;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Block, Borders, Chart, Clear, Dataset, Gauge, GraphType, Paragraph, Wrap,
};
use ratatui::{Frame, symbols};

use crate::app::AppState;
use crate::constants::{STATE_OFF, STATE_ON, TARGET_FLOW_MAX};
use crate::data::register_name;

pub fn render_ui(frame: &mut Frame, app: &AppState) {
    let mut constraints = vec![
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(6),
    ];
    if app.show_debug {
        let debug_lines = app
            .status
            .as_ref()
            .map_or(1, |status| status.registers.len().div_ceil(2));
        let debug_height = u16::try_from(debug_lines + 2).unwrap_or(u16::MAX);
        constraints.push(Constraint::Length(debug_height));
    }
    constraints.push(Constraint::Length(3));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.area());

    let mut index = 0;
    render_header(frame, chunks[index], app);
    index += 1;
    render_status(frame, chunks[index], app);
    index += 1;
    render_flow_chart(frame, chunks[index], app);
    index += 1;
    render_speed_chart(frame, chunks[index], app);
    index += 1;
    render_filters(frame, chunks[index], app);
    index += 1;
    if app.show_debug {
        render_debug(frame, chunks[index], app);
        index += 1;
    }
    render_help(frame, chunks[index]);

    if app.input_mode {
        render_target_popup(frame, app);
    }
}

fn render_header(frame: &mut Frame, area: Rect, _app: &AppState) {
    let title = Line::from(vec![Span::styled(
        "Quick 6101A2 Monitor",
        Style::default()
            .fg(Color::LightMagenta)
            .add_modifier(Modifier::BOLD),
    )]);

    let paragraph = Paragraph::new(title).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_status(frame: &mut Frame, area: Rect, app: &AppState) {
    let (state_text, state_style) = match app.status.as_ref().map(|status| status.state) {
        Some(STATE_ON) => (
            "ON",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Some(STATE_OFF) => (
            "OFF",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        _ => ("--", Style::default().fg(Color::Gray)),
    };

    let (connection_text, connection_style) = if app.connected {
        (
            "Connected",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (
            "Disconnected",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    };

    let target_flow = app.status.as_ref().map_or(0, |status| status.target_flow);
    let real_flow = app.status.as_ref().map_or(0, |status| status.real_flow);
    let mode_label = if app.simulate { "SIM" } else { "LIVE" };
    let mode_color = if app.simulate {
        Color::Yellow
    } else {
        Color::Blue
    };

    let line = Line::from(vec![
        Span::styled("State: ", Style::default().fg(Color::Gray)),
        Span::styled(state_text, state_style),
        Span::raw("  "),
        Span::styled("Link: ", Style::default().fg(Color::Gray)),
        Span::styled(connection_text, connection_style),
        Span::raw("  "),
        Span::styled("Target Flow: ", Style::default().fg(Color::Gray)),
        Span::raw(format!("{target_flow} m3/h")),
        Span::raw("  "),
        Span::styled("Real Flow: ", Style::default().fg(Color::Gray)),
        Span::raw(format!("{real_flow} m3/h")),
        Span::raw("  "),
        Span::styled("Mode: ", Style::default().fg(Color::Gray)),
        Span::styled(
            mode_label,
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            if app.read_only {
                "Read-only"
            } else {
                "Writable"
            },
            Style::default()
                .fg(if app.read_only {
                    Color::Yellow
                } else {
                    Color::Green
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let paragraph = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Status")
            .border_style(Style::default().fg(Color::LightMagenta)),
    );
    frame.render_widget(paragraph, area);
}

fn render_flow_chart(frame: &mut Frame, area: Rect, app: &AppState) {
    let (real_data, target_data) = split_series(&app.flow_history);
    let (min_tick, max_tick) = chart_bounds(&real_data, area);

    let datasets = vec![
        Dataset::default()
            .name("Target")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::LightYellow))
            .graph_type(GraphType::Line)
            .data(&target_data),
        Dataset::default()
            .name("Real")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::LightCyan))
            .graph_type(GraphType::Line)
            .data(&real_data),
    ];

    let chart_title = Line::from(vec![
        Span::styled("Flow (m3/h)", Style::default().fg(Color::White)),
        Span::raw("  "),
        Span::styled("Real", Style::default().fg(Color::LightCyan)),
        Span::raw("/"),
        Span::styled("Target", Style::default().fg(Color::LightYellow)),
    ]);

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(chart_title)
                .border_style(Style::default().fg(Color::LightCyan)),
        )
        .x_axis(
            Axis::default()
                .bounds([min_tick, max_tick])
                .labels(vec![Span::from("-"), Span::from("+")]),
        )
        .y_axis(
            Axis::default()
                .bounds([f64::from(0), f64::from(TARGET_FLOW_MAX)])
                .labels(vec![
                    Span::from("0"),
                    Span::from(format!("{TARGET_FLOW_MAX}")),
                ]),
        );

    frame.render_widget(chart, area);
}

fn render_speed_chart(frame: &mut Frame, area: Rect, app: &AppState) {
    let data: Vec<(f64, f64)> = app.speed_history.iter().copied().collect();
    let (min_tick, max_tick) = chart_bounds(&data, area);
    let max_speed = data
        .iter()
        .map(|(_, value)| *value)
        .fold(0.0, f64::max)
        .max(100.0);

    let datasets = vec![
        Dataset::default()
            .name("RPM")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::LightGreen))
            .graph_type(GraphType::Line)
            .data(&data),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Fan Speed (RPM)")
                .border_style(Style::default().fg(Color::LightYellow)),
        )
        .x_axis(
            Axis::default()
                .bounds([min_tick, max_tick])
                .labels(vec![Span::from("-"), Span::from("+")]),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, max_speed])
                .labels(vec![Span::from("0"), Span::from(format!("{max_speed:.0}"))]),
        );

    frame.render_widget(chart, area);
}

fn render_filters(frame: &mut Frame, area: Rect, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    render_filter_gauge(
        frame,
        chunks[0],
        "P-Filter",
        app.status.as_ref().map(|s| s.p_filter_total),
        app.status.as_ref().map(|s| s.p_filter_limit),
    );
    render_filter_gauge(
        frame,
        chunks[1],
        "M-Filter",
        app.status.as_ref().map(|s| s.m_filter_total),
        app.status.as_ref().map(|s| s.m_filter_limit),
    );
    render_filter_gauge(
        frame,
        chunks[2],
        "C-Filter",
        app.status.as_ref().map(|s| s.c_filter_total),
        app.status.as_ref().map(|s| s.c_filter_limit),
    );
}

fn render_filter_gauge(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    total: Option<u16>,
    limit: Option<u16>,
) {
    let total = f64::from(total.unwrap_or(0));
    let value = f64::from(limit.unwrap_or(0));
    let ratio = if value > 0.0 {
        (total / value).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(label)
                .border_style(Style::default().fg(Color::LightGreen)),
        )
        .gauge_style(Style::default().fg(Color::LightGreen))
        .ratio(ratio)
        .label(format!("{total:.0}/{value:.0} km3"));
    frame.render_widget(gauge, area);
}

fn render_debug(frame: &mut Frame, area: Rect, app: &AppState) {
    let mut lines = Vec::new();
    if let Some(status) = &app.status {
        let column_width: usize = 36;
        let mut row_spans: Vec<Span> = Vec::new();
        for (index, value) in status.registers.iter().enumerate() {
            let name = u16::try_from(index)
                .ok()
                .and_then(register_name)
                .unwrap_or("-");
            let address = format!("0x{index:04X} ");
            let rest = format!("{name:<12} 0x{value:04X} {value:>5}");
            let entry_len = address.len() + rest.len();

            row_spans.push(Span::styled(
                address,
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ));
            row_spans.push(Span::raw(rest));

            if index % 2 == 0 {
                let padding = column_width.saturating_sub(entry_len);
                row_spans.push(Span::raw(" ".repeat(padding)));
            } else {
                lines.push(Line::from(row_spans));
                row_spans = Vec::new();
            }
        }
        if !row_spans.is_empty() {
            lines.push(Line::from(row_spans));
        }
    } else {
        lines.push(Line::from("No register data yet"));
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Registers")
            .border_style(Style::default().fg(Color::LightGreen)),
    );
    frame.render_widget(paragraph, area);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let text = Line::from(vec![
        Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" toggle power  "),
        Span::styled("←/→", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" adjust target flow  "),
        Span::styled("t", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" type target  "),
        Span::styled("d", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" registers  "),
        Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" quit"),
    ]);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Controls")
                .border_style(Style::default().fg(Color::LightMagenta)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_target_popup(frame: &mut Frame, app: &AppState) {
    let area = centered_rect(60, 20, frame.area());
    let buffer = if app.input_buffer.is_empty() {
        "_".to_string()
    } else {
        app.input_buffer.clone()
    };

    let content = vec![
        Line::from(Span::styled(
            "Type target flow",
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Value: ", Style::default().fg(Color::Gray)),
            Span::styled(
                buffer,
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" m3/h"),
        ]),
        Line::from(""),
        Line::from("Enter to apply, Esc to cancel"),
    ];

    frame.render_widget(Clear, area);
    let paragraph = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Target Flow")
            .border_style(Style::default().fg(Color::LightMagenta)),
    );
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn chart_bounds(data: &[(f64, f64)], area: Rect) -> (f64, f64) {
    if data.is_empty() {
        return (0.0, 1.0);
    }
    let max_tick = data.last().map_or(0.0, |(x, _)| *x).max(1.0);
    let window = area.width.saturating_sub(2).max(1) as usize;
    let window_ticks = u32::try_from(window.min(data.len().max(1)))
        .ok()
        .map_or(f64::from(u32::MAX), f64::from);
    let min_tick = if max_tick > window_ticks {
        max_tick - window_ticks
    } else {
        0.0
    };
    (min_tick, max_tick)
}

type Series = Vec<(f64, f64)>;

fn split_series(series: &VecDeque<(f64, f64)>) -> (Series, Series) {
    let mut real = Vec::new();
    let mut target = Vec::new();
    for (index, point) in series.iter().enumerate() {
        if index % 2 == 0 {
            real.push(*point);
        } else {
            target.push(*point);
        }
    }
    (real, target)
}
