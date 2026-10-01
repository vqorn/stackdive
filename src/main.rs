mod content;
mod game;
mod map;
mod rng;
mod ui;

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use game::{Action, Game, Status};
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

const HELP: &str = "stackdive: a tiny roguelike for your terminal.
Dive down eight layers of the stack and squash the bugs.

Usage
  stackdive              a new random dungeon
  stackdive --daily      today's dungeon, the same for everyone
  stackdive --seed <N>   a dungeon you can play again (a number or any word)
  stackdive --demo       watch a bot play

Options
  -h, --help       show this help
  -V, --version    show the version

Keys
  arrows, hjkl, wasd  move         yubn  diagonal       .  wait
  >  take the stairs               c  drink coffee      o  Stack Overflow answer
  ?  help and legend               q  quit

Walk into a bug to hit it. Your result is printed when the run ends,
so you can paste it to your friends.
";

struct Options {
    seed: u64,
    label: String,
    /// What to type to play the same dungeon again.
    replay: String,
    demo: bool,
}

fn parse_args() -> Result<Options, String> {
    let mut seed: Option<(u64, String)> = None;
    let mut demo = false;
    let mut daily = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("stackdive {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--daily" => daily = true,
            "--demo" => demo = true,
            "--seed" => {
                let text = args
                    .next()
                    .ok_or("--seed needs a number or a word, e.g. --seed 42")?;
                seed = Some((rng::seed_from_text(&text), format!("seed {}", text.trim())));
            }
            other => return Err(format!("unknown argument '{other}' (see stackdive --help)")),
        }
    }
    if daily && seed.is_some() {
        return Err("use either --daily or --seed, not both".to_string());
    }
    if daily {
        let (y, m, d) = rng::today_utc();
        return Ok(Options {
            seed: rng::daily_seed(),
            label: format!("daily {y:04}-{m:02}-{d:02}"),
            replay: "stackdive --daily".to_string(),
            demo,
        });
    }
    let (seed, label) = seed.unwrap_or_else(|| {
        let s = rng::random_seed();
        (s, format!("seed {s}"))
    });
    let replay = format!("stackdive --seed {}", label.trim_start_matches("seed "));
    Ok(Options {
        seed,
        label,
        replay,
        demo,
    })
}

fn restore_terminal() {
    let mut out = io::stdout();
    let _ = execute!(out, Show, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<TerminalGuard> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            default(info);
        }));
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

enum Input {
    Act(Action),
    Help,
    Quit,
    Force,
    Other,
}

fn read_input(key: KeyEvent) -> Input {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Input::Force;
    }
    match key.code {
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('a') => Input::Act(Action::Move(-1, 0)),
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('d') => Input::Act(Action::Move(1, 0)),
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => Input::Act(Action::Move(0, -1)),
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => Input::Act(Action::Move(0, 1)),
        KeyCode::Char('y') => Input::Act(Action::Move(-1, -1)),
        KeyCode::Char('u') => Input::Act(Action::Move(1, -1)),
        KeyCode::Char('b') => Input::Act(Action::Move(-1, 1)),
        KeyCode::Char('n') => Input::Act(Action::Move(1, 1)),
        KeyCode::Char('.') | KeyCode::Char(' ') => Input::Act(Action::Wait),
        KeyCode::Char('>') | KeyCode::Enter => Input::Act(Action::Descend),
        KeyCode::Char('c') => Input::Act(Action::Coffee),
        KeyCode::Char('o') => Input::Act(Action::Scroll),
        KeyCode::Char('?') | KeyCode::F(1) => Input::Help,
        KeyCode::Char('q') | KeyCode::Esc => Input::Quit,
        _ => Input::Other,
    }
}

/// Waits for a key press; ignores key releases and mouse noise.
fn next_key() -> io::Result<Option<KeyEvent>> {
    loop {
        match event::read()? {
            Event::Key(k) if k.kind != KeyEventKind::Release => return Ok(Some(k)),
            Event::Resize(..) => return Ok(None),
            _ => {}
        }
    }
}

fn play(game: &mut Game, label: &str, demo: bool) -> io::Result<()> {
    let mut out = io::BufWriter::new(io::stdout());
    let mut ui = ui::Ui::default();
    let mut steps = 0u32;
    // Not documented on purpose: lets the demo GIF in the README run fast.
    let demo_delay: u64 = std::env::var("STACKDIVE_DEMO_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(70);
    loop {
        ui::draw(&mut out, game, &ui, label, demo)?;
        out.flush()?;
        if game.status != Status::Playing {
            if demo {
                std::thread::sleep(Duration::from_millis(1800));
                return Ok(());
            }
            if let Some(_key) = next_key()? {
                return Ok(());
            }
            continue;
        }
        if demo {
            if event::poll(Duration::from_millis(demo_delay))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind != KeyEventKind::Release
                        && matches!(read_input(k), Input::Quit | Input::Force)
                    {
                        return Ok(());
                    }
                }
            }
            steps += 1;
            let action = game.bot_action();
            game.act(action);
            if steps > 20_000 {
                return Ok(());
            }
            continue;
        }
        let Some(key) = next_key()? else { continue };
        if ui.help > 0 {
            ui.help = if ui.help == 1 { 2 } else { 0 };
            continue;
        }
        if ui.confirm_quit {
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
                return Ok(());
            }
            ui.confirm_quit = false;
            continue;
        }
        match read_input(key) {
            Input::Act(action) => {
                game.act(action);
            }
            Input::Help => ui.help = 1,
            Input::Quit => ui.confirm_quit = true,
            Input::Force => return Ok(()),
            Input::Other => {}
        }
    }
}

fn main() {
    let options = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("stackdive: {e}");
            std::process::exit(2);
        }
    };
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        eprintln!("stackdive: needs an interactive terminal");
        std::process::exit(2);
    }
    let mut game = Game::new(options.seed);
    let result = {
        let _guard = match TerminalGuard::enter() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("stackdive: could not set up the terminal: {e}");
                std::process::exit(2);
            }
        };
        play(&mut game, &options.label, options.demo)
    };
    if let Err(e) = result {
        eprintln!("stackdive: {e}");
        std::process::exit(1);
    }
    println!("\n{}", game.summary(&options.label));
    println!("Play it again: {}", options.replay);
}
