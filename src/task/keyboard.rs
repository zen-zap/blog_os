// in src/task/keyboard.rs

use conquer_once::spin::OnceCell;
use core::{
	iter::Scan,
	pin::Pin,
	task::{Context, Poll}
};
use crossbeam_queue::ArrayQueue;
use futures_util::{
	task::AtomicWaker,
	stream::Stream,
	stream::StreamExt
};
use crate::{
	warn,
	debug,
	print
};
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};

/// Used to store the tasks from the Interrupt Handler
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

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
			warn!("SCANCODE_QUEUE full; dropping keyboard input");
		} else {
			// you get an input, you wake up the SCANCODE_WAKER
			SCANCODE_WAKER.wake();
			// the waker in turn notifies the executor
		}
	} else {
		warn!("scancode queue uninitialized!");
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
			.try_init_once(|| ArrayQueue::new(200))
			.expect("ScancodeStream::new should only be called once");

		ScancodeStream { _private: () }
	}

	// Next, we need to make something so that we can poll continuously from the stream
	// .. no this is not the Future type since it stops once Ready, here we need more
	// since they are keystrokes, they keep coming
	// Made Stream trait to handle this
}

/// Waker for scancode stream
///
/// The poll_next implementation stores the current waker in this static,
/// and the add_scancode function calls wake() on this when a new scancode is added.
///
/// AtomicWaker --> can be modified safely in concurrent scenarios
static SCANCODE_WAKER: AtomicWaker = AtomicWaker::new();

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

///  Loops until it can decode a full key from a scancode stream
///
/// This is a reusable async function that any task can call to await
/// the next complete key press
pub async fn read_key(
	scancodes: &mut ScancodeStream,
	keyboard: &mut Keyboard<layouts::Us104Key, ScancodeSet1>
) -> DecodedKey {
	// we'll loop internally until we have enough scancodes from the stream to completely decode
	// the key
	loop {
		let scancode = scancodes.next().await.expect("Failed to fetch next scancode");
		// check if full key or not
		if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
			if let Some(key) = keyboard.process_keyevent(key_event) {
				return key;
			}
		}
		// if we're here, it means that the scancode was not a full key yet.
		// we'll await the next scancode in the next iteration
	}
}