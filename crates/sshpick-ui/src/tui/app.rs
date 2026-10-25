use std::io;
use std::time::Duration;

use crossterm::event::{self, Event};
use sshpick_core::{Outcome, Picker, Previews};

use super::input;
use super::screen::Screen;
use super::terminal::Tty;

const BUSY_POLL: Duration = Duration::from_millis(15);
const IDLE_POLL: Duration = Duration::from_secs(3600);

pub fn run(screen: &Screen, picker: &mut Picker, previews: &mut Previews) -> io::Result<Outcome> {
    let mut tty = Tty::enter()?;
    let outcome = events(&mut tty, screen, picker, previews);
    tty.leave()?;
    outcome
}

fn events(
    tty: &mut Tty,
    screen: &Screen,
    picker: &mut Picker,
    previews: &mut Previews,
) -> io::Result<Outcome> {
    loop {
        let selected = picker.selected().cloned();
        let slot = selected.as_ref().map(|host| previews.fetch(host));
        tty.terminal
            .draw(|frame| screen.draw(frame, picker, slot))?;

        let timeout = if previews.busy() {
            BUSY_POLL
        } else {
            IDLE_POLL
        };
        if !event::poll(timeout)? {
            continue;
        }
        if let Event::Key(event) = event::read()?
            && let Some(outcome) = input::key(event).and_then(|key| picker.handle(key))
        {
            return Ok(outcome);
        }
    }
}
