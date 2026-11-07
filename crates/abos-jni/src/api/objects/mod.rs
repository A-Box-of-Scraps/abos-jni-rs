mod jobject;
pub use self::jobject::JObject;
pub(crate) use self::jobject::JObjectAPI;

mod jthrowable;
pub use self::jthrowable::*;

mod jstack_trace_element;
pub use self::jstack_trace_element::*;

mod jclass;
pub use self::jclass::*;

mod jclass_loader;
pub use self::jclass_loader::*;

mod jchar_sequence;
pub use self::jchar_sequence::*;

mod jstring;
pub use self::jstring::*;

mod jcollection;
pub use self::jcollection::*;

mod jset;
pub use self::jset::*;

mod jiterator;
pub use self::jiterator::*;

mod jmap;
pub use self::jmap::*;

mod jlist;
pub use self::jlist::*;

mod jbytebuffer;
pub use self::jbytebuffer::*;

mod jthread;
pub use self::jthread::*;

/// Primitive Array types
mod jobject_array;
pub use self::jobject_array::JObjectArray;
pub(crate) use self::jobject_array::JObjectArrayAPI;

mod type_array;
pub use self::type_array::TypeArray;
pub(crate) use self::type_array::TypeArraySealed;

/// Primitive Array types
mod jprimitive_array;
pub(crate) use self::jprimitive_array::JPrimitiveArrayAPI_jboolean;
pub use self::jprimitive_array::{
	AsJArrayRaw, JBooleanArray, JByteArray, JCharArray, JDoubleArray, JFloatArray, JIntArray, JLongArray,
	JPrimitiveArray, JShortArray,
};

#[doc(hidden)]
#[deprecated(
	since = "0.22.0",
	note = "Please use `abos_jni::JValue*` instead of `abos_jni::objects::JValue*`."
)]
pub use crate::api::jvalue::*;

#[doc(hidden)]
#[deprecated(
	since = "0.22.0",
	note = "Please use ID types under `abos_jni::ids::*` instead of `abos_jni::objects::*`."
)]
pub use crate::ids::*;

#[doc(hidden)]
#[deprecated(
	since = "0.22.0",
	note = "Please use reference types under `abos_jni::refs::*` instead of `abos_jni::objects::*`."
)]
pub use crate::refs::*;

#[doc(hidden)]
#[deprecated(
	since = "0.22.0",
	note = "Please use array elements types under `abos_jni::elements::*` instead of `abos_jni::objects::*`."
)]
pub use crate::elements::*;
