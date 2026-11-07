use std::char::{CharTryFromError, DecodeUtf16Error};
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;

use log::trace;

use crate::signature::JavaType;
use crate::{
	errors::{CharToJavaError, Error, Result},
	objects::JObject,
	signature::Primitive,
	sys::{JNI_TRUE, jboolean, jbyte, jchar, jdouble, jfloat, jint, jlong, jshort, jvalue},
};

#[cfg(doc)]
use crate::api::env::Env;

/// A Java owned local reference or primitive value.
///
/// This type is used for values returned from Java method calls. If the Java
/// method returns an object reference, it will take the form of an owned
/// [`JObject`].
///
/// See also [`JValue`], which is used for Java method call parameters. It is
/// different from this type in that it *borrows* an object reference instead
/// of owning one.
#[allow(
	missing_docs,
	reason = "Each variant names the JNI value type stored in its payload."
)]
#[derive(Debug)]
pub enum JValueOwned<'local> {
	Object(JObject<'local>),
	Byte(jbyte),
	Char(jchar),
	Short(jshort),
	Int(jint),
	Long(jlong),
	Bool(jboolean),
	Float(jfloat),
	Double(jdouble),
	Void,
}

/// A Java borrowed local reference or primitive value.
///
/// This type is used for parameters passed to Java method calls. If the Java
/// method is to be passed an object reference, it takes the form of a borrowed
/// <code>&[JObject]</code>.
///
/// See also [`JValueOwned`], which is used for Java method return values. It is
/// different from this type in that it *owns* an object reference instead
/// of borrowing one.
#[allow(
	missing_docs,
	reason = "Each variant names the JNI value type stored in its payload."
)]
#[derive(Clone, Copy, Debug)]
pub enum JValue<'obj_ref> {
	Object(&'obj_ref JObject<'obj_ref>),
	Byte(jbyte),
	Char(jchar),
	Short(jshort),
	Int(jint),
	Long(jlong),
	Bool(jboolean),
	Float(jfloat),
	Double(jdouble),
	Void,
}

mod borrowed;
mod chars;
mod conversions;
mod owned;
pub use chars::*;
