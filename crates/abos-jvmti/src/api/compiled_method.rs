//! Callback-scoped decoding of the implementation-specific `jvmticmlr.h` format.

use std::{collections::HashSet, ffi::c_void, marker::PhantomData, slice};

use crate::sys;

/// A malformed compiled-method metadata record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompiledMethodError {
	/// A record links back to an already visited record.
	Cycle,
	/// An array length is negative or exceeds Rust's slice size limit.
	InvalidLength,
	/// A nonempty array has a null pointer.
	NullArray,
}

impl std::fmt::Display for CompiledMethodError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "invalid compiled-method metadata: {self:?}")
	}
}
impl std::error::Error for CompiledMethodError {}

/// The identifying fields of a record header, without its borrowed link pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompiledMethodHeader {
	/// Implementation-defined record kind.
	pub kind: sys::jvmtiCMLRKind,
	/// Metadata major version (independent of the JVMTI interface version).
	pub major: sys::jint,
	/// Metadata minor version.
	pub minor: sys::jint,
}

/// One inlined method and its corresponding bytecode index.
///
/// Copying a method ID does not keep its declaring class loaded. Resolve symbols
/// while the class is live, or retain the class separately before using the ID later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineFrame {
	/// Opaque JVM method identifier, not a JNI object reference.
	pub method: sys::jmethodID,
	/// Bytecode index supplied by the VM, including any negative sentinel value.
	pub bci: sys::jint,
}

/// A borrowed PC-to-stack mapping, in the order supplied by the VM.
#[derive(Clone, Copy, Debug)]
pub struct PcStack<'callback> {
	pc: usize,
	methods: &'callback [sys::jmethodID],
	bcis: &'callback [sys::jint],
}

impl PcStack<'_> {
	/// Native PC as an address only. This does not keep compiled code alive.
	#[must_use]
	pub const fn pc(&self) -> usize {
		self.pc
	}

	/// Pair every method ID with its bytecode index without reordering frames.
	pub fn frames(&self) -> impl ExactSizeIterator<Item = InlineFrame> + '_ {
		self.methods
			.iter()
			.zip(self.bcis)
			.map(|(&method, &bci)| InlineFrame { method, bci })
	}

	/// Copy the mapping into Rust-owned storage.
	#[must_use]
	pub fn to_owned(&self) -> OwnedPcStack {
		OwnedPcStack {
			pc: self.pc,
			frames: self.frames().collect(),
		}
	}
}

/// Rust-owned stack data. PCs and method IDs are non-owning identifiers;
/// code unloading and class unloading can invalidate them independently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedPcStack {
	/// Native PC address, never dereferenced by this API.
	pub pc: usize,
	/// Method/BCI pairs in VM order.
	pub frames: Vec<InlineFrame>,
}

/// One decoded record. Unknown kinds and unsupported versions expose only their
/// header; their payload is never read or guessed.
#[derive(Debug)]
pub enum CompiledMethodRecord<'callback> {
	/// Version 1.0 inline information, including empty PC and frame arrays.
	Inline(Vec<PcStack<'callback>>),
	/// A version 1.0 diagnostic record. The bytes need not be NUL-terminated.
	Dummy(&'callback [std::ffi::c_char; 50]),
	/// An unrecognized kind with a supported header version.
	Unknown(CompiledMethodHeader),
	/// A version other than exactly 1.0, even for a recognized kind.
	UnsupportedVersion(CompiledMethodHeader),
}

/// A deep copy of all recognized record payloads. No callback memory is retained.
/// Unknown payloads cannot be copied because their size is unspecified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedCompiledMethodRecord {
	/// Copied inline PC mappings.
	Inline(Vec<OwnedPcStack>),
	/// Copied diagnostic bytes.
	Dummy([std::ffi::c_char; 50]),
	/// Header only for an unknown kind.
	Unknown(CompiledMethodHeader),
	/// Header only for an unsupported version.
	UnsupportedVersion(CompiledMethodHeader),
}

impl CompiledMethodRecord<'_> {
	/// Deep-copy recognized payloads using Rust's allocator, not JVMTI's allocator.
	#[must_use]
	pub fn to_owned(&self) -> OwnedCompiledMethodRecord {
		match self {
			Self::Inline(pcs) => OwnedCompiledMethodRecord::Inline(pcs.iter().map(PcStack::to_owned).collect()),
			Self::Dummy(message) => OwnedCompiledMethodRecord::Dummy(**message),
			Self::Unknown(header) => OwnedCompiledMethodRecord::Unknown(*header),
			Self::UnsupportedVersion(header) => OwnedCompiledMethodRecord::UnsupportedVersion(*header),
		}
	}
}

/// A callback-scoped iterator over `jvmticmlr.h` records, not standard JVMTI data.
///
/// Null `compile_info` is an empty iterator. Errors terminate iteration. Unknown
/// kinds and unsupported versions are yielded as header-only records and traversal
/// continues through the common header's `next` link. This requires that the VM
/// preserves that common header ABI even for extensions.
///
/// Borrowed records cannot escape the decoding scope:
///
/// ```compile_fail
/// use abos_jvmti::{CompiledMethodRecord, CompiledMethodRecords};
/// unsafe fn escape(info: *const std::ffi::c_void) -> CompiledMethodRecord<'static> {
///     unsafe { CompiledMethodRecords::with(info, |mut records| records.next().unwrap().unwrap()) }
/// }
/// ```
///
/// No JVMTI calls or deallocations are performed. This API needs no attachment or
/// phase beyond the active callback. Receiving `CompiledMethodLoad` requires
/// `can_generate_compiled_method_load_events` and enabling that event. Never unwind
/// through the native callback boundary.
#[derive(Debug)]
pub struct CompiledMethodRecords<'callback> {
	next: *const sys::jvmtiCompiledMethodLoadRecordHeader,
	visited: HashSet<*const sys::jvmtiCompiledMethodLoadRecordHeader>,
	lifetime: PhantomData<&'callback sys::jvmtiCompiledMethodLoadRecordHeader>,
}

impl CompiledMethodRecords<'_> {
	/// Access metadata within a scope that cannot return callback-borrowed data.
	/// Return [`OwnedCompiledMethodRecord`] values to retain decoded information.
	///
	/// # Safety
	/// For VM-supplied data, call only inside the `CompiledMethodLoad` callback that
	/// supplied `compile_info`, after establishing that the VM uses the `jvmticmlr.h`
	/// format (for example, `HotSpot`). Standard JVMTI does not guarantee that format.
	/// Synthetic records may also be decoded if they satisfy the same layout,
	/// readability, and immutability requirements below for the entire scope.
	/// Every non-null link must point to an aligned, initialized common header.
	/// Recognized version 1.0 records must have their full corresponding layout.
	/// Non-null arrays must be aligned, initialized, and readable for their declared
	/// nonnegative lengths. All memory must remain immutable throughout `body`.
	/// Null arrays and negative lengths are reported, but arbitrary bad pointers
	/// cannot be validated. Do not free any of this VM-owned callback memory.
	pub unsafe fn with<R>(
		compile_info: *const c_void,
		body: impl for<'callback> FnOnce(CompiledMethodRecords<'callback>) -> R,
	) -> R {
		body(CompiledMethodRecords {
			next: compile_info.cast(),
			visited: HashSet::new(),
			lifetime: PhantomData,
		})
	}
}

impl<'callback> Iterator for CompiledMethodRecords<'callback> {
	type Item = Result<CompiledMethodRecord<'callback>, CompiledMethodError>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.next.is_null() {
			return None;
		}
		let pointer: *const sys::jvmtiCompiledMethodLoadRecordHeader = self.next;
		self.next = std::ptr::null();
		if !self.visited.insert(pointer) {
			return Some(Err(CompiledMethodError::Cycle));
		}
		// SAFETY: with's contract guarantees readable common headers and recognized
		// payloads for the entire callback scope. Decoding checks counts/null arrays.
		let (next, record) = unsafe { ((*pointer).next, decode(pointer)) };
		if record.is_ok() {
			self.next = next;
		}
		Some(record)
	}
}

impl std::iter::FusedIterator for CompiledMethodRecords<'_> {}

unsafe fn array<'a, T>(pointer: *const T, count: sys::jint) -> Result<&'a [T], CompiledMethodError> {
	let count: usize = usize::try_from(count).map_err(|_| CompiledMethodError::InvalidLength)?;
	if count == 0 {
		return Ok(&[]);
	}
	if count > (isize::MAX as usize) / std::mem::size_of::<T>() {
		return Err(CompiledMethodError::InvalidLength);
	}
	if pointer.is_null() {
		return Err(CompiledMethodError::NullArray);
	}
	// SAFETY: The caller guarantees initialized, aligned arrays of declared length.
	Ok(unsafe { slice::from_raw_parts(pointer, count) })
}

unsafe fn decode<'a>(
	pointer: *const sys::jvmtiCompiledMethodLoadRecordHeader,
) -> Result<CompiledMethodRecord<'a>, CompiledMethodError> {
	// SAFETY: The caller supplies a readable header for the callback scope.
	let raw: &sys::jvmtiCompiledMethodLoadRecordHeader = unsafe { &*pointer };
	let header = CompiledMethodHeader {
		kind: raw.kind,
		major: raw.majorinfoversion,
		minor: raw.minorinfoversion,
	};
	if header.major != sys::JVMTI_CMLR_MAJOR_VERSION || header.minor != sys::JVMTI_CMLR_MINOR_VERSION {
		return Ok(CompiledMethodRecord::UnsupportedVersion(header));
	}
	match header.kind {
		sys::JVMTI_CMLR_INLINE_INFO => {
			// SAFETY: A recognized v1.0 kind guarantees the full inline-record layout.
			let inline: &sys::jvmtiCompiledMethodLoadInlineRecord =
				unsafe { &*pointer.cast::<sys::jvmtiCompiledMethodLoadInlineRecord>() };
			// SAFETY: Nested arrays obey the caller's callback-memory contract.
			let pcs: &[sys::PCStackInfo] = unsafe { array(inline.pcinfo, inline.numpcs)? };
			let mut stacks: Vec<PcStack<'_>> = Vec::with_capacity(pcs.len());
			for pc in pcs {
				// SAFETY: Both arrays contain numstackframes initialized elements.
				let (methods, bcis) = unsafe {
					(
						array(pc.methods, pc.numstackframes)?,
						array(pc.bcis, pc.numstackframes)?,
					)
				};
				stacks.push(PcStack {
					pc: pc.pc as usize,
					methods,
					bcis,
				});
			}
			Ok(CompiledMethodRecord::Inline(stacks))
		}
		sys::JVMTI_CMLR_DUMMY => {
			// SAFETY: A recognized v1.0 kind guarantees the full dummy-record layout.
			let dummy: &sys::jvmtiCompiledMethodLoadDummyRecord =
				unsafe { &*pointer.cast::<sys::jvmtiCompiledMethodLoadDummyRecord>() };
			Ok(CompiledMethodRecord::Dummy(&dummy.message))
		}
		_ => Ok(CompiledMethodRecord::Unknown(header)),
	}
}
