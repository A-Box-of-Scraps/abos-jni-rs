//! Synthetic compiled-method records, including unsupported and malformed data.
#![allow(unsafe_code, reason = "Tests construct callback records in stable local storage.")]

use std::{ffi::c_void, ptr};

use abos_jvmti::{CompiledMethodError, CompiledMethodRecord, CompiledMethodRecords, OwnedCompiledMethodRecord, sys};

const fn header(kind: sys::jvmtiCMLRKind) -> sys::jvmtiCompiledMethodLoadRecordHeader {
	sys::jvmtiCompiledMethodLoadRecordHeader {
		kind,
		majorinfoversion: 1,
		minorinfoversion: 0,
		next: ptr::null_mut(),
	}
}

// SAFETY: Callers keep the complete record graph and nested arrays live and immutable.
unsafe fn copy(pointer: *const c_void) -> Result<Vec<OwnedCompiledMethodRecord>, CompiledMethodError> {
	// SAFETY: Forward the caller's callback-memory contract; only owned data escapes.
	unsafe {
		CompiledMethodRecords::with(pointer, |records| {
			records.map(|record| record.map(|r| r.to_owned())).collect()
		})
	}
}

#[test]
fn null_and_empty_inline_data() {
	// SAFETY: Null denotes an empty list.
	assert_eq!(unsafe { copy(ptr::null()) }, Ok(vec![]));
	let inline = sys::jvmtiCompiledMethodLoadInlineRecord {
		header: header(sys::JVMTI_CMLR_INLINE_INFO),
		..Default::default()
	};
	// SAFETY: Zero-length arrays may be null.
	assert_eq!(
		// SAFETY: The local record has valid storage; null arrays are handled.
		unsafe { copy((&raw const inline).cast()) },
		Ok(vec![OwnedCompiledMethodRecord::Inline(vec![])])
	);
}

#[test]
fn mixed_records_decode_frames_and_copy_nested_storage() {
	let owned: Vec<OwnedCompiledMethodRecord> = {
		let mut methods: [sys::jmethodID; 2] = [ptr::without_provenance_mut(0x100), ptr::without_provenance_mut(0x200)];
		let mut bcis = [42, -1];
		let mut pcs: [sys::PCStackInfo; 2] = [
			sys::PCStackInfo {
				pc: ptr::without_provenance_mut(0x1234),
				numstackframes: 2,
				methods: methods.as_mut_ptr(),
				bcis: bcis.as_mut_ptr(),
			},
			sys::PCStackInfo::default(),
		];
		let mut inline = sys::jvmtiCompiledMethodLoadInlineRecord {
			header: header(sys::JVMTI_CMLR_INLINE_INFO),
			numpcs: 2,
			pcinfo: pcs.as_mut_ptr(),
		};
		let mut dummy = sys::jvmtiCompiledMethodLoadDummyRecord {
			header: header(sys::JVMTI_CMLR_DUMMY),
			message: [65; 50],
		};
		dummy.header.next = &raw mut inline.header;
		let mut unknown: sys::jvmtiCompiledMethodLoadRecordHeader = header(sys::jvmtiCMLRKind::MAX);
		unknown.next = &raw mut dummy.header;
		// A recognized kind with an unsupported version has only a header. Reading
		// its inline payload would be out of bounds, so version checks must come first.
		let mut unsupported: sys::jvmtiCompiledMethodLoadRecordHeader = header(sys::JVMTI_CMLR_INLINE_INFO);
		unsupported.majorinfoversion = 2;
		unsupported.next = &raw mut unknown;
		// SAFETY: All linked records and arrays remain in scope and immutable.
		unsafe { copy((&raw const unsupported).cast()) }.expect("valid records")
	};
	assert!(matches!(owned[0], OwnedCompiledMethodRecord::UnsupportedVersion(h) if h.major == 2));
	assert!(matches!(owned[1], OwnedCompiledMethodRecord::Unknown(h) if h.kind == sys::jvmtiCMLRKind::MAX));
	assert_eq!(owned[2], OwnedCompiledMethodRecord::Dummy([65; 50]));
	let OwnedCompiledMethodRecord::Inline(pcs) = &owned[3] else {
		panic!("inline record")
	};
	assert_eq!(pcs.len(), 2);
	assert_eq!(pcs[0].pc, 0x1234);
	assert_eq!(pcs[0].frames[0].method as usize, 0x100);
	assert_eq!(pcs[0].frames[0].bci, 42);
	assert_eq!(pcs[0].frames[1].method as usize, 0x200);
	assert_eq!(pcs[0].frames[1].bci, -1);
	assert_eq!(pcs[1].frames, []);
}

#[test]
fn unsupported_minor_version_does_not_read_payload() {
	let mut record: sys::jvmtiCompiledMethodLoadRecordHeader = header(sys::JVMTI_CMLR_DUMMY);
	record.minorinfoversion = 1;
	// SAFETY: An unsupported version requires only the common header.
	let records: Vec<OwnedCompiledMethodRecord> = unsafe { copy((&raw const record).cast()) }.expect("header only");
	assert!(matches!(records[0], OwnedCompiledMethodRecord::UnsupportedVersion(h) if h.minor == 1));
}

#[test]
fn malformed_arrays_are_reported() {
	for (count, expected) in [
		(-1, CompiledMethodError::InvalidLength),
		(1, CompiledMethodError::NullArray),
	] {
		let mut inline = sys::jvmtiCompiledMethodLoadInlineRecord {
			header: header(sys::JVMTI_CMLR_INLINE_INFO),
			numpcs: count,
			pcinfo: ptr::null_mut(),
		};
		// SAFETY: Invalid counts/null pointers are explicitly supported error cases.
		assert_eq!(unsafe { copy((&raw const inline).cast()) }, Err(expected));
		let mut pc = sys::PCStackInfo {
			numstackframes: count,
			..Default::default()
		};
		inline.numpcs = 1;
		inline.pcinfo = &raw mut pc;
		// SAFETY: The outer array is valid; invalid nested arrays are reported.
		assert_eq!(unsafe { copy((&raw const inline).cast()) }, Err(expected));
	}
	let mut method: sys::jmethodID = ptr::null_mut();
	let mut pc = sys::PCStackInfo {
		numstackframes: 1,
		methods: &raw mut method,
		..Default::default()
	};
	let inline = sys::jvmtiCompiledMethodLoadInlineRecord {
		header: header(sys::JVMTI_CMLR_INLINE_INFO),
		numpcs: 1,
		pcinfo: &raw mut pc,
	};
	// SAFETY: The methods array is readable; the missing BCI array is reported.
	assert_eq!(
		// SAFETY: The local record has valid storage; null arrays are handled.
		unsafe { copy((&raw const inline).cast()) },
		Err(CompiledMethodError::NullArray)
	);
}

#[test]
fn cycles_terminate_and_iterator_is_fused() {
	let mut record: sys::jvmtiCompiledMethodLoadRecordHeader = header(99);
	record.next = &raw mut record;
	// SAFETY: The self-link remains valid; the decoder reports cycles.
	unsafe {
		CompiledMethodRecords::with((&raw const record).cast(), |mut records| {
			assert!(matches!(records.next(), Some(Ok(CompiledMethodRecord::Unknown(_)))));
			assert!(matches!(records.next(), Some(Err(CompiledMethodError::Cycle))));
			assert!(records.next().is_none());
			assert!(records.next().is_none());
		});
	}
}

#[test]
fn borrowed_pc_access_pairs_frames() {
	let mut method: sys::jmethodID = ptr::null_mut();
	let mut bci = 7;
	let mut pc = sys::PCStackInfo {
		pc: ptr::without_provenance_mut(0x1000),
		numstackframes: 1,
		methods: &raw mut method,
		bcis: &raw mut bci,
	};
	let inline = sys::jvmtiCompiledMethodLoadInlineRecord {
		header: header(2),
		numpcs: 1,
		pcinfo: &raw mut pc,
	};
	// SAFETY: Every record and nested array is valid for this scope.
	unsafe {
		CompiledMethodRecords::with((&raw const inline).cast(), |mut records| {
			let CompiledMethodRecord::Inline(pcs) = records.next().expect("record").expect("valid") else {
				panic!("inline")
			};
			assert_eq!(pcs[0].pc(), 0x1000);
			assert_eq!(pcs[0].frames().len(), 1);
			assert_eq!(pcs[0].frames().next().expect("frame").bci, 7);
		});
	}
}
