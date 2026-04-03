use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};

pub fn run_live_loop<F>(interval: Duration, freeze: bool, render: F) -> Result<(), String>
where
    F: FnMut() -> Result<String, String>,
{
    run_live_loop_until(interval, freeze, render, || Ok(true))
}

pub fn run_live_loop_until<F, C>(
    interval: Duration,
    freeze: bool,
    render: F,
    should_continue: C,
) -> Result<(), String>
where
    F: FnMut() -> Result<String, String>,
    C: FnMut() -> Result<bool, String>,
{
    run_live_loop_with_event_handler(interval, freeze, render, should_continue, |_| Ok(false))
}

pub fn run_live_loop_with_event_handler<F, C, H>(
    interval: Duration,
    freeze: bool,
    mut render: F,
    mut should_continue: C,
    mut handle_event: H,
) -> Result<(), String>
where
    F: FnMut() -> Result<String, String>,
    C: FnMut() -> Result<bool, String>,
    H: FnMut(&Event) -> Result<bool, String>,
{
    if freeze {
        print!("{}", render()?);
        io::stdout()
            .flush()
            .map_err(|err| format!("Failed to flush output: {err}"))?;
        return Ok(());
    }

    let mut stdout = io::stdout();
    let _terminal = LiveTerminalGuard::enter(&mut stdout)?;

    loop {
        clear_screen(&mut stdout)?;
        write_live_frame(&mut stdout, &render()?)?;
        stdout
            .flush()
            .map_err(|err| format!("Failed to flush output: {err}"))?;

        let deadline = Instant::now() + interval;
        while Instant::now() < deadline {
            if !should_continue()? {
                return Ok(());
            }
            if event::poll(Duration::from_millis(100))
                .map_err(|err| format!("Failed to read terminal input: {err}"))?
            {
                let next =
                    event::read().map_err(|err| format!("Failed to read terminal input: {err}"))?;
                if should_exit_live_view(&next) {
                    return Ok(());
                }
                if handle_event(&next)? {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

pub fn clear_screen(stdout: &mut io::Stdout) -> Result<(), String> {
    write!(stdout, "\x1B[2J\x1B[H").map_err(|err| format!("Failed to clear terminal screen: {err}"))
}

pub fn write_live_frame(stdout: &mut io::Stdout, frame: &str) -> Result<(), String> {
    let normalized = frame.trim_end_matches('\n').replace('\n', "\r\n");
    write!(stdout, "{normalized}").map_err(|err| format!("Failed to write live frame: {err}"))
}

pub fn should_exit_live_view(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(key)
            if key.kind != KeyEventKind::Release
                && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
    )
}

pub struct LiveTerminalGuard;

impl LiveTerminalGuard {
    pub fn enter(stdout: &mut io::Stdout) -> Result<Self, String> {
        terminal::enable_raw_mode().map_err(|err| format!("Failed to enable raw mode: {err}"))?;
        execute!(stdout, EnterAlternateScreen, Hide)
            .map_err(|err| format!("Failed to initialize terminal UI: {err}"))?;
        Ok(Self)
    }
}

impl Drop for LiveTerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
