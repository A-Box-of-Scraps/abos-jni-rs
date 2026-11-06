use crate::{
	Env,
	descriptors::Desc,
	errors::Result,
	objects::{Auto, IntoAuto as _, JClass, LoaderContext},
	strings::JNIStr,
};

// SAFETY: Lookup resolves the class by name and returns a typed local class reference retained by Auto.
unsafe impl<'local, T> Desc<'local, JClass<'local>> for T
where
	T: AsRef<JNIStr>,
{
	type Output = Auto<'local, JClass<'local>>;

	fn lookup(self, env: &mut Env<'local>) -> Result<Self::Output> {
		Ok(LoaderContext::None.find_class(env, self.as_ref(), false)?.auto())
	}
}

// Note: we don't implement `Desc<JClass>` for `&JObject` as a simple cast would
// make it a lot easier to mistakenly pass an object instance in places where a
// class is required.
