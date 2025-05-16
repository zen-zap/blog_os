// in src/task/keyboard.rs

use conquer_once::spin::OnceCell;
use core::iter::Scan;
use crossbeam_queue::ArrayQueue;

/// Used to store the tasks from the Interrupt Handler
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

use crate::println;

/// Called by the keyboard interrupt handler
///
/// Not callable from main.rs
/// pub(crate) limits visibility to lib.rs
///
/// Must not block or allocate!
pub(crate) fn add_scancode(scancode: u8) {
	// get a reference to the initialized queue
	if let Ok(queue) = SCANCODE_QUEUE.try_get() {
		if let Err(_) = queue.push(scancode) {
			println!("WARNING: SCANCODE_QUEUE full; dropping keyboard input");
		} else {
			// you get an input, you wake up the SCANCODE_WAKER
			SCANCODE_WAKER.wake();
			// the waker in turn notifies the executor
		}
	} else {
		println!("WARNING: scancode queue uninitialized!");
	}
}

/// To initialize the SCANCODE_QUEUE and read the scancodes in the queue in an
/// asynchronous way, we make a scancode stream
pub struct ScancodeStream {
	/// purpose of this field is to prevent construction of this outside of the module
	_private: (),
}

impl ScancodeStream {
	/// made for exclusive creation of ScancodeStream since it is a private struct
	pub fn new() -> Self {
		SCANCODE_QUEUE
			.try_init_once(|| ArrayQueue::new(100))
			.expect("ScancodeStream::new should only be called once");

		ScancodeStream { _private: () }
	}

	// Next, we need to make something so that we can poll continuously from the stream
	// .. no this is not the Future type since it stops once Ready, here we need more
	// since they are keystrokes, they keep coming
	// Made Stream trait to handle this
}

use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;

/// Waker for scancode stream
///
/// The poll_next implementation stores the current waker in this static,
/// and the add_scancode function calls wake() on this when a new scancode is added.
///
/// AtomicWaker --> can be modified safely in concurrent scenarios
static SCANCODE_WAKER: AtomicWaker = AtomicWaker::new();

use core::pin::Pin;
use core::task::{Context, Poll};

impl Stream for ScancodeStream {
	type Item = u8;

	fn poll_next(
		self: Pin<&mut Self>,
		cx: &mut Context,
	) -> Poll<Option<u8>> {
		let queue = SCANCODE_QUEUE.try_get().expect("scancode not initialized");

		// fast path
		if let Some(scancode) = queue.pop() {
			return Poll::Ready(Some(scancode));
			// don't need the waker if it's not pending
		}

		// the queue might be potentially empty here .. since the interrupt handler
		// could've filled it immediately after the check
		// .. hence we have to register the waker before the second check
		// We get a guarantee that we get a wakeup for any scancodes pushed after the check

		SCANCODE_WAKER.register(&cx.waker());

		match queue.pop() {
			Some(scancode) => {
				// it succeeds so need for the SCANCODE_WAKER anymore
				SCANCODE_WAKER.take();
				Poll::Ready(Some(scancode))
			},
			None => Poll::Pending, // returned with a registered waker
		}
	}
}

use crate::print;
use futures_util::stream::StreamExt;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};

pub async fn print_keypresses() {
	let mut scancodes = ScancodeStream::new();
	let mut keyboard = Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::Ignore);

	while let Some(scancode) = scancodes.next().await {
		if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
			if let Some(key) = keyboard.process_keyevent(key_event) {
				match key {
					DecodedKey::RawKey(key) => {
						// ignore raw keys -- if you want .. you don't wanna print them .. looks
						// ugly
					},
					DecodedKey::Unicode(character) => print!("{}", character),
				}
			}
		}
	}
}
