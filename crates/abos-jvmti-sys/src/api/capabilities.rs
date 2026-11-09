/// Capability bitfields, including the reserved bits from `jvmti.h`.
///
/// Start with [`Default::default`] so reserved bits remain zero, then use the
/// named setters. No C compiler is needed to build this representation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct jvmtiCapabilities {
	pub(crate) words: [u32; 4],
}
