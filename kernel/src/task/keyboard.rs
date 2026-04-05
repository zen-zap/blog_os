//! in kernel/src/task/keyboard.rs
//! 
//! Asynchronous Keyboard Handling
//!
//! This module provides a non-blocking, asynchronous stream of keyboard
//! scancodes. It uses a lock-free queue to safely transfer data from the
//! hardware interrupt handler to the kernel's async task executor.

use crate::{print, warn};
use conquer_once::spin::OnceCell;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::ArrayQueue;
use futures_util::{stream::Stream, stream::StreamExt, task::AtomicWaker};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

/// Used to store the raw scancodes from the hardware Interrupt Handler.
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

/// Waker for the scancode stream.
///
/// The `poll_next` implementation stores the current executor waker in this static,
/// and the `add_scancode` function calls `wake()` on it when a new scancode is pushed.
/// Being an `AtomicWaker`, it can be modified safely in concurrent scenarios.
static SCANCODE_WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the keyboard interrupt handler.
///
/// Limits visibility to `lib.rs` via `pub(crate)` so it cannot be called from main.
/// This function must never block or allocate, as it runs in an interrupt context!
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if queue.push(scancode).is_err() {
            warn!("SCANCODE_QUEUE full; dropping keyboard input");
        } else {
            // We received an input, so we wake up the SCANCODE_WAKER.
            // The waker in turn notifies the async executor to poll the stream again.
            SCANCODE_WAKER.wake();
        }
    } else {
        warn!("SCANCODE_QUEUE uninitialized!");
    }
}

/// An asynchronous stream of hardware scancodes.
///
/// Initializes the `SCANCODE_QUEUE` and allows the kernel to read the scancodes
/// in an asynchronous, non-blocking way. We implement the `Stream` trait instead
/// of a standard `Future` because keystrokes are a continuous flow of data, not
/// a single event that finishes once ready.
pub struct ScancodeStream {
    /// The purpose of this private field is to prevent construction of this struct
    /// from outside of this module.
    _private: (),
}

impl ScancodeStream {
    /// Creates a new `ScancodeStream`.
    ///
    /// This should exclusively be used to create the stream since the struct has
    /// private fields. It initializes the backing array queue on its first call.
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(200))
            .expect("ScancodeStream::new should only be called once");

        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE.try_get().expect("scancode queue not initialized");

        // Fast path: if there is a scancode, grab it immediately without touching the waker.
        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        // The queue might be potentially empty here. However, since the interrupt handler
        // could have filled it immediately after our check, we must register the waker
        // BEFORE our second check. This guarantees we get a wakeup for any scancodes
        // pushed after the check.
        SCANCODE_WAKER.register(cx.waker());

        match queue.pop() {
            Some(scancode) => {
                // We succeeded in grabbing a scancode, so we don't need the waker anymore
                // for this specific poll cycle.
                SCANCODE_WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending, // Returns pending with the waker securely registered
        }
    }
}

/// A background kernel task that continuously prints decoded keystrokes.
pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    );

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::RawKey(_key) => {} // Ignore raw keys like Shift/Ctrl for printing
                    DecodedKey::Unicode(character) => print!("{}", character),
                }
            }
        }
    }
}

/// Loops until it can decode a full key from a scancode stream.
///
/// This is a reusable async function that any task (like a shell) can call to await
/// the next complete key press. It loops internally until enough scancodes are fetched
/// from the stream to completely decode a single key.
pub async fn read_key(
    scancodes: &mut ScancodeStream,
    keyboard: &mut Keyboard<layouts::Us104Key, ScancodeSet1>,
) -> DecodedKey {
    loop {
        let scancode = scancodes.next().await.expect("Failed to fetch next scancode");

        // Feed the scancode into the state machine. If it returns a full key, return it.
        // Otherwise, loop and await the next scancode.
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                return key;
            }
        }
    }
}