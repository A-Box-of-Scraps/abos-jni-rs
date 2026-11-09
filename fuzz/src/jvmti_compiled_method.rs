//! Bounded record graphs with an independent index-based traversal oracle.

use std::ptr;

use abos_jvmti::{
	CompiledMethodError, CompiledMethodHeader, CompiledMethodRecord, CompiledMethodRecords, InlineFrame,
	OwnedCompiledMethodRecord, OwnedPcStack, sys,
};

use crate::Input;

type Decoded = Result<OwnedCompiledMethodRecord, CompiledMethodError>;

// Boxes keep headers stable while the graph is assembled. Nested Vec buffers are
// never resized after their addresses are installed in a record.
enum Record {
	Header(Box<sys::jvmtiCompiledMethodLoadRecordHeader>),
	Dummy(Box<sys::jvmtiCompiledMethodLoadDummyRecord>),
	Inline(Box<sys::jvmtiCompiledMethodLoadInlineRecord>),
}

impl Record {
	fn header(&mut self) -> *mut sys::jvmtiCompiledMethodLoadRecordHeader {
		match self {
			Self::Header(header) => &raw mut **header,
			Self::Dummy(record) => &raw mut record.header,
			Self::Inline(record) => &raw mut record.header,
		}
	}
}

#[derive(Default)]
struct Storage {
	pcs: Vec<Vec<sys::PCStackInfo>>,
	methods: Vec<Vec<sys::jmethodID>>,
	bcis: Vec<Vec<i32>>,
}

fn inline(input: &mut Input<'_>, storage: &mut Storage) -> (sys::jvmtiCompiledMethodLoadInlineRecord, Decoded) {
	let mode = input.byte() % 4;
	let count = usize::from(input.byte() % 9);
	let mut pcs = Vec::new();
	let mut expected = Vec::new();
	let mut error = match mode {
		1 => Some(CompiledMethodError::InvalidLength),
		2 => Some(CompiledMethodError::NullArray),
		_ => None,
	};
	if mode == 0 {
		for _ in 0..count {
			let frame_mode = input.byte() % 5;
			let frames = usize::from(input.byte() % 9);
			let pc = input.int() as u32 as usize;
			let mut methods = Vec::new();
			let mut bcis = Vec::new();
			let mut model = Vec::new();
			for _ in 0..frames {
				// Opaque identifiers are copied, never dereferenced or sent to a VM.
				let method = ptr::without_provenance_mut(input.int() as u32 as usize);
				let bci = input.int();
				methods.push(method);
				bcis.push(bci);
				model.push(InlineFrame { method, bci });
			}
			let numstackframes = match frame_mode {
				1 => -1,
				4 => 0,
				_ => frames as i32,
			};
			if frame_mode == 1 {
				error.get_or_insert(CompiledMethodError::InvalidLength);
			} else if matches!(frame_mode, 2 | 3) && frames != 0 {
				error.get_or_insert(CompiledMethodError::NullArray);
			}
			pcs.push(sys::PCStackInfo {
				pc: ptr::without_provenance_mut(pc),
				numstackframes,
				methods: if frame_mode == 2 {
					ptr::null_mut()
				} else {
					methods.as_mut_ptr()
				},
				bcis: if frame_mode == 3 {
					ptr::null_mut()
				} else {
					bcis.as_mut_ptr()
				},
			});
			if frame_mode == 4 {
				model.clear();
			}
			expected.push(OwnedPcStack { pc, frames: model });
			storage.methods.push(methods);
			storage.bcis.push(bcis);
		}
	}
	let record = sys::jvmtiCompiledMethodLoadInlineRecord {
		header: sys::jvmtiCompiledMethodLoadRecordHeader::default(),
		numpcs: match mode {
			1 => -1,
			2 => 1,
			_ => pcs.len() as i32,
		},
		pcinfo: if mode != 0 { ptr::null_mut() } else { pcs.as_mut_ptr() },
	};
	storage.pcs.push(pcs);
	(
		record,
		error.map_or_else(|| Ok(OwnedCompiledMethodRecord::Inline(expected)), Err),
	)
}

/// Decode up to 16 records, each with at most eight PCs and eight frames per PC.
pub fn run(data: &[u8]) {
	let mut input = Input(data);
	let count = usize::from(input.byte() % 17);
	let start = usize::from(input.byte()) % (count + 1);
	let mut records = Vec::new();
	let mut links = Vec::new();
	let mut model = Vec::new();
	let mut storage = Storage::default();
	for _ in 0..count {
		let kind = input.byte() % 5;
		links.push(usize::from(input.byte()) % (count + 1));
		let mut header = sys::jvmtiCompiledMethodLoadRecordHeader {
			kind: sys::JVMTI_CMLR_INLINE_INFO,
			majorinfoversion: 1,
			minorinfoversion: 0,
			next: ptr::null_mut(),
		};
		let (record, expected) = match kind {
			0 | 1 => {
				if kind == 0 {
					header.kind = 3 + u32::from(input.byte());
				} else {
					header.kind = 1 + u32::from(input.byte() % 2);
					header.majorinfoversion = input.int();
					header.minorinfoversion = input.int();
				}
				if kind == 1 && header.majorinfoversion == 1 && header.minorinfoversion == 0 {
					header.minorinfoversion = 1;
				}
				let view = CompiledMethodHeader {
					kind: header.kind,
					major: header.majorinfoversion,
					minor: header.minorinfoversion,
				};
				let expected = if kind == 0 {
					OwnedCompiledMethodRecord::Unknown(view)
				} else {
					OwnedCompiledMethodRecord::UnsupportedVersion(view)
				};
				// Header-only allocation catches accidental reads of unsupported payloads.
				(Record::Header(Box::new(header)), Ok(expected))
			}
			2 => {
				header.kind = sys::JVMTI_CMLR_DUMMY;
				let message = std::array::from_fn(|_| input.byte() as std::ffi::c_char);
				(
					Record::Dummy(Box::new(sys::jvmtiCompiledMethodLoadDummyRecord { header, message })),
					Ok(OwnedCompiledMethodRecord::Dummy(message)),
				)
			}
			_ => {
				let (mut record, expected) = inline(&mut input, &mut storage);
				record.header = header;
				(Record::Inline(Box::new(record)), expected)
			}
		};
		records.push(record);
		model.push(expected);
	}
	let mut pointers: Vec<_> = records.iter_mut().map(Record::header).collect();
	pointers.push(ptr::null_mut());
	for (index, &link) in links.iter().enumerate() {
		// SAFETY: Every pointer addresses a distinct live, aligned boxed header.
		unsafe { (*pointers[index]).next = pointers[link] };
	}
	let mut expected = Vec::new();
	let mut seen = vec![false; count];
	let mut index = start;
	while index != count {
		if seen[index] {
			expected.push(Err(CompiledMethodError::Cycle));
			break;
		}
		seen[index] = true;
		expected.push(model[index].clone());
		if model[index].is_err() {
			break;
		}
		index = links[index];
	}
	// SAFETY: The synthetic graph and matching payload layouts remain immutable
	// and live throughout the scope. Positive non-null arrays fit their backing
	// allocations; null arrays and negative counts are supported error cases.
	let actual = unsafe {
		CompiledMethodRecords::with(pointers[start].cast(), |mut iter| {
			let mut actual = Vec::new();
			for _ in 0..=count {
				let Some(record) = iter.next() else {
					break;
				};
				if let Ok(CompiledMethodRecord::Inline(pcs)) = &record {
					for pc in pcs {
						assert_eq!(pc.frames().len(), pc.frames().count());
						assert_eq!(pc.to_owned().pc, pc.pc());
					}
				}
				actual.push(record.map(|record| record.to_owned()));
			}
			assert!(iter.next().is_none(), "bounded graph must terminate");
			assert!(iter.next().is_none(), "iterator must remain exhausted");
			actual
		})
	};
	drop(records);
	drop(storage);
	assert_eq!(actual, expected, "owned copies must survive backing storage");
}
