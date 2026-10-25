use std::io;
use std::time::Duration;

use crossterm::event::{self, Event};
use sshpick_core::{Outcome, Picker, Previews};

use super::input;
use super::plan::Space;
use super::screen::Screen;
use super::terminal::Tty;

const BUSY_POLL: Duration = Duration::from_millis(15);
const IDLE_POLL: Duration = Duration::from_secs(3600);

/// Erase the block before returning: `exec` comes after this.
pub fn run(screen: &Screen, picker: &mut Picker, previews: &mut Previews) -> io::Result<Outcome> {
    let mut space = Space::detect();
    let mut tty = Tty::enter(space, screen.plan(space, picker))?;
    let outcome = events(&mut tty, &mut space, screen, picker, previews);
    tty.leave()?;
    outcome
}

fn events(
    tty: &mut Tty,
    space: &mut Space,
    screen: &Screen,
    picker: &mut Picker,
    previews: &mut Previews,
) -> io::Result<Outcome> {
    loop {
        let plan = screen.plan(*space, picker);
        tty.fit(*space, plan)?;
        let selected = picker.selected().cloned();
        let slot = selected.as_ref().map(|host| previews.fetch(host));
        tty.draw(|frame| screen.draw(frame, picker, slot, plan))?;

        let timeout = if previews.busy() {
            BUSY_POLL
        } else {
            IDLE_POLL
        };
        if !event::poll(timeout)? {
            continue;
        }
        // A paste is one frame, and a window drag is one resize, to the last size.
        loop {
            match event::read()? {
                Event::Key(event) => {
                    if let Some(outcome) = input::key(event).and_then(|key| picker.handle(key)) {
                        return Ok(outcome);
                    }
                }
                Event::Resize(cols, rows) => *space = Space::reported(cols, rows),
                _ => {}
            }
            if !event::poll(Duration::ZERO)? {
                break;
            }
        }
    }
}
