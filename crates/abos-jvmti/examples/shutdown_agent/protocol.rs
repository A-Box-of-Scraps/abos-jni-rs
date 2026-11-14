//! Example-only shutdown protocol. No JNI/JVMTI calls occur in the admission gate.

use std::sync::atomic::{AtomicUsize, Ordering};

use abos_jvmti::{Error, Result, sys};

const CLOSED: usize = 1 << (usize::BITS - 1);
const COUNT_MASK: usize = CLOSED - 1;

pub struct Gate {
	// Closing and admission share one atomic word, so a late increment cannot escape draining.
	state: AtomicUsize,
}

impl Gate {
	pub const fn new() -> Self {
		Self {
			state: AtomicUsize::new(0),
		}
	}

	pub fn enter(&self) -> Option<Admission<'_>> {
		self.state
			.try_update(Ordering::AcqRel, Ordering::Acquire, |state| {
				if state & CLOSED != 0 || state == COUNT_MASK {
					None
				} else {
					Some(state + 1)
				}
			})
			.ok()
			.map(|_| Admission { gate: self })
	}

	// Callbacks may request closure, but must leave disabling, waiting, and disposal to
	// a coordinator in a permitted context. In particular they must not wait for themselves.
	pub fn close(&self) {
		self.state.fetch_or(CLOSED, Ordering::AcqRel);
	}

	pub fn drained(&self) -> bool {
		// Acquire observes all work published by departing admissions, not just the count.
		self.state.load(Ordering::Acquire) == CLOSED
	}
}

#[must_use = "Keep this admission alive through every access to agent resources and the environment."]
pub struct Admission<'a> {
	gate: &'a Gate,
}

impl Drop for Admission<'_> {
	fn drop(&mut self) {
		self.gate.state.fetch_sub(1, Ordering::Release);
	}
}

// These operations are supplied by the example's attached, live-phase coordinator.
// This is not a public safe abstraction over arbitrary copies of Env.
pub trait Operations {
	fn disable_delivery(&mut self) -> Result<()>;
	fn release_resources(&mut self) -> Result<()>;
	fn dispose(&mut self) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
	Pending,
	Complete,
}

#[derive(Clone, Copy)]
enum Phase {
	Running,
	Draining,
	Failed(Error),
	Disposed,
}

pub struct Shutdown<'a> {
	gate: &'a Gate,
	phase: Phase,
}

impl<'a> Shutdown<'a> {
	pub const fn new(gate: &'a Gate) -> Self {
		Self {
			gate,
			phase: Phase::Running,
		}
	}

	pub fn poll(&mut self, operations: &mut impl Operations) -> Result<Progress> {
		match self.phase {
			Phase::Failed(error) => return Err(error),
			Phase::Disposed => return Ok(Progress::Complete),
			Phase::Running => {
				self.gate.close();
				// Poison before external operations: even a panic must not allow an unsafe retry.
				self.phase = Phase::Failed(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
				if let Err(error) = operations.disable_delivery() {
					self.phase = Phase::Failed(error);
					return Err(error);
				}
				self.phase = Phase::Draining;
			}
			Phase::Draining => {}
		}
		if !self.gate.drained() {
			return Ok(Progress::Pending);
		}
		// This example owns no worker threads or suspended threads. A real agent must
		// also stop/join its workers and resume suspended threads before freeing resources.
		self.phase = Phase::Failed(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL));
		if let Err(error) = operations.release_resources().and_then(|()| operations.dispose()) {
			self.phase = Phase::Failed(error);
			return Err(error);
		}
		self.phase = Phase::Disposed;
		Ok(Progress::Complete)
	}
}
