use std::sync::mpsc::{Sender, Receiver, channel, SendError};
use std::thread::{self, JoinHandle, sleep};
use std::time::Duration;
use std::fmt::Display;
use crossterm::{
    ExecutableCommand,
    QueueableCommand,
    cursor,
    terminal,
    style,
    event,
    event::{
        Event,
        KeyCode,
    }
};
use std::io::{Write, Stdout, stdout, self};
use ez_input::{AnyHandle, EzEvent};

static PROMPT_X: u16 = 1;
static PROMPT_Y: u16 = 1;

static ORIGIN_X: u16 = 4;
static ORIGIN_Y: u16 = 3;

#[inline]
fn draw_cursor(out: &mut Stdout, pos: u16, delta: i32) -> Result<(), std::io::Error> {
    let old_cursor_y: i32 = ORIGIN_Y as i32 + pos as i32 - delta;
    out.queue(cursor::MoveTo(ORIGIN_X - 3, old_cursor_y.try_into().unwrap_or_default()))?;
    out.queue(style::Print("  "))?;

    let cursor_y = ORIGIN_Y + pos;
    out.queue(cursor::MoveTo(ORIGIN_X - 3, cursor_y))?;
    out.queue(style::Print("=>"))?;

    Ok(())
}

#[inline]
fn draw_menu(out: &mut Stdout, prompt: &str, items: &[String]) -> Result<(), std::io::Error> {
    out.queue(terminal::Clear(terminal::ClearType::All))?;
    out.queue(cursor::MoveTo(PROMPT_X, PROMPT_Y))?;
    out.queue(style::Print(prompt))?;

    out.queue(cursor::MoveTo(ORIGIN_X, ORIGIN_Y))?;
    for c in items {
        out.queue(style::Print(c.to_string()))?;
        out.queue(cursor::MoveToNextLine(1))?;
        out.queue(cursor::MoveToColumn(ORIGIN_X))?;
    }

    if items.is_empty() {
        out.queue(style::Print("<No options supplied>"))?;
    }

    Ok(())
}

fn pad_input(tx: Sender<EzEvent>) {
    let mut handle = AnyHandle::open();
    loop {
        if let Some(ev) = handle.get_event_blocking() {
            match ev {
                EzEvent::DirectionUp | EzEvent::DirectionDown | EzEvent::South(_) => (),
                _ => continue,
            };

            let ret = tx.send(ev);
            if ret.is_err() {
                return;
            }
        }
    }
}

#[inline]
pub fn pick<T: Send + Sync + Display>(prompt: String, entry_rx: Receiver<T>) -> Result<T, io::Error> {
    let mut out = stdout();

    // prepare for f a n c y display
    out.execute(terminal::EnterAlternateScreen)?;
    out.execute(cursor::Hide)?;
    terminal::enable_raw_mode()?;

    let (tx, pad_rx) = channel();
    thread::spawn(|| pad_input(tx));

    let mut entries = Vec::new();
    let mut entries_disp = Vec::new();

    let mut cursor_pos: i32 = 0;
    let mut cursor_change: i32 = 0;
    let mut picked = false;
    let mut redraw_cursor = true;
    let mut redraw_menu = true;
    while !picked {
        if let Ok(new_entry) = entry_rx.try_recv() {
            entries_disp.push(new_entry.to_string());
            entries.push(new_entry);
            redraw_menu = true;
        }

        if let Ok(event) = pad_rx.try_recv() {
            match event {
                EzEvent::South(val)     => picked = true,
                EzEvent::DirectionUp    => cursor_change = -1,
                EzEvent::DirectionDown  => cursor_change = 1,
                _ => (),
            }
        }

        if let Ok(tmp) = event::poll(Duration::from_millis(10)) {
            if tmp {
                if let Ok(Event::Key(key)) = event::read() {
                    match key.code {
                        KeyCode::Enter  => picked = true,
                        KeyCode::Down   => cursor_change = 1,
                        KeyCode::Up     => cursor_change = -1,
                        _ => (),
                    }
                }
            }
        }

        if cursor_change != 0 {
            redraw_cursor = true;
        }

        if redraw_menu {
            draw_menu(&mut out, &prompt, &entries_disp)?;
            redraw_cursor = true;
        }

        cursor_pos += cursor_change;
        cursor_pos = cursor_pos.clamp(0, entries.len().saturating_sub(1).try_into().unwrap_or_default());

        if redraw_cursor{
            if let Ok(pos) = cursor_pos.try_into() {
                draw_cursor(&mut out, pos, cursor_change)?;
            }
        }

        cursor_change = 0;

        // sometimes there are no options and the user *still* pressed the confirm key
        if entries.is_empty() && picked {
            picked = false;
        }


        if redraw_cursor || redraw_menu {
            out.flush()?;
        }

        redraw_cursor = false;
        redraw_menu = false;
        sleep(Duration::from_millis(50));
    }

    // clean up after ourselves
    out.execute(cursor::Show)?;
    out.execute(terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    Ok(entries.swap_remove(cursor_pos.try_into().unwrap_or(0)))
}

pub struct Picker<T: Send + Sync + Display> {
    entry_tx: Sender<T>,
    join_handle: JoinHandle<Result<T, io::Error>>,
}

impl <T: Send + Sync + Display + 'static> Picker<T> {
    #[inline]
    pub fn is_chosen(&mut self) -> bool {
        self.join_handle.is_finished()
    }

    #[inline]
    pub fn wait_choice(self) -> Result<T, io::Error> {
        // That thread cannot panic
        self.join_handle.join().unwrap()
    }

    #[inline]
    pub fn add_option(&mut self, new_opt: T) -> Result<(), SendError<T>> {
        self.entry_tx.send(new_opt)
    }

    #[inline]
    pub fn add_options<I>(&mut self, iter: I) -> Result<(), SendError<T>>
    where
        I: IntoIterator<Item = T>,
    {
        let iterator = iter.into_iter();
        for entry in iterator {
            self.entry_tx.send(entry)?;
        }
        Ok(())
    }

    #[inline]
    pub fn new(prompt: String) -> Picker<T> {
        let (entry_tx, entry_rx) = channel();
        Picker {
            entry_tx,
            join_handle: thread::spawn(move || pick(prompt, entry_rx)),
        }
    }
}
