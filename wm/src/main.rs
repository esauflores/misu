use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use wm::{Command, Config, input::Action, ipc, state::State};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = match Config::from_args(std::env::args().skip(1)) {
        Ok(Command::Run(config)) => config,
        Ok(Command::Msg(words)) => std::process::exit(ipc::client(&words)),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    init_logging();

    let mut event_loop: EventLoop<State> = EventLoop::try_new()?;
    let display: Display<State> = Display::new()?;
    let mut state = State::new(&mut event_loop, display, config);

    wm::winit::init(&mut event_loop, &mut state)?;
    let ipc_path = ipc::listen(&mut event_loop, &state.socket_name)?;

    // Children connect to us, not to the host, and can drive us.
    // Safety: still single-threaded here.
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);
        std::env::set_var(ipc::ENV, &ipc_path);
    }
    tracing::info!(
        "listening on {:?}, control socket {}, {} seats",
        state.socket_name,
        ipc_path.display(),
        state.seats.list.len()
    );

    if let Some(command) = state.config.command.clone() {
        state.run(0, Action::Spawn(command));
    }

    // After each turn, tell subscribers what changed.
    event_loop.run(None, &mut state, State::publish)?;
    let _ = std::fs::remove_file(&ipc_path);
    Ok(())
}

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
