// Java lookup names are independent of the Rust identifiers used by the bindings.
abos_jni::bind_java_type! {
    pub ExplicitNames => "example.ExplicitNames",
    fields {
        dollar_value {
            name = "$value",
            sig = jint,
            get = dollar_value,
            set = set_dollar_value,
        },
        currency { name = "¥", sig = jint },
        identifier_field { name = javaField, sig = jint },
    },
    methods {
        fn foo_bar { name = "foo$bar", sig = () -> jint },
        fn currency_method { name = "¥", sig = () -> jint },
        fn identifier_method { name = javaMethod, sig = () -> jint },
    },
    native_methods {
        pub static fn native_dollar { name = "native$dollar", sig = () -> jint },
        static fn native_identifier { name = nativeIdentifier, sig = () -> jint },
    },
}

impl ExplicitNamesNativeInterface for ExplicitNamesAPI {
    type Error = abos_jni::errors::Error;

    fn native_dollar<'local>(
        _env: &mut abos_jni::Env<'local>,
        _class: abos_jni::objects::JClass<'local>,
    ) -> Result<abos_jni::sys::jint, Self::Error> {
        Ok(42)
    }

    fn native_identifier<'local>(
        _env: &mut abos_jni::Env<'local>,
        _class: abos_jni::objects::JClass<'local>,
    ) -> Result<abos_jni::sys::jint, Self::Error> {
        Ok(17)
    }
}

fn main() {}
