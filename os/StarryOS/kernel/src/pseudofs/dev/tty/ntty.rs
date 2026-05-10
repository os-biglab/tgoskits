use alloc::sync::Arc;
use core::time::Duration;

use axpoll::PollSet;
use lazy_static::lazy_static;

use super::{
    Tty,
    terminal::ldisc::{ProcessMode, TtyConfig, TtyRead, TtyWrite},
};

pub type NTtyDriver = Tty<Console, Console>;

#[derive(Clone, Copy)]
pub struct Console;
impl TtyRead for Console {
    fn read(&mut self, buf: &mut [u8]) -> usize {
        ax_hal::console::read_bytes(buf)
    }
}
impl TtyWrite for Console {
    fn write(&self, buf: &[u8]) {
        ax_hal::console::write_bytes(buf);
    }
}

lazy_static! {
    /// The default TTY device.
    pub static ref N_TTY: Arc<NTtyDriver> = new_n_tty();
    static ref CONSOLE_INPUT_SOURCE: Arc<PollSet> = Arc::new(PollSet::new());
}

fn handle_console_input_irq(_irq_num: usize) {
    let events = ax_hal::console::handle_irq();
    if events.intersects(
        ax_hal::console::ConsoleIrqEvent::RX_READY
            | ax_hal::console::ConsoleIrqEvent::RX_ERROR
            | ax_hal::console::ConsoleIrqEvent::OVERRUN,
    ) {
        CONSOLE_INPUT_SOURCE.wake();
    }
}

fn new_n_tty() -> Arc<NTtyDriver> {
    Tty::new(
        Arc::default(),
        TtyConfig {
            reader: Console,
            writer: Console,
            process_mode: console_irq_mode().unwrap_or_else(console_polling_mode),
        },
    )
}

fn console_irq_mode() -> Option<ProcessMode> {
    let irq = ax_hal::console::irq_num()?;
    if !ax_hal::irq::register(irq, handle_console_input_irq) {
        warn!("Failed to register console IRQ handler for irq {irq}, falling back to polling mode");
        return None;
    }

    ax_hal::console::set_input_irq_enabled(true);
    Some(ProcessMode::InterruptDriven(CONSOLE_INPUT_SOURCE.clone()))
}

/// Fallback for platforms without a console IRQ (e.g. x86-qemu-q35).
///
/// Spawns a background task that wakes the reader every millisecond so that
/// signal characters (Ctrl+C, Ctrl+Z, …) are delivered even when no process
/// is blocked on a `read()` syscall.
fn console_polling_mode() -> ProcessMode {
    let source = Arc::new(PollSet::new());
    let source_clone = source.clone();
    ax_task::spawn_with_name(
        move || loop {
            source_clone.wake();
            ax_task::sleep(Duration::from_millis(1));
        },
        "console-poll".into(),
    );
    ProcessMode::InterruptDriven(source)
}
