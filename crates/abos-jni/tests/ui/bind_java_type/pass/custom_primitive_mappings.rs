// Exercise every custom primitive kind in constructor/method arguments, returns, and fields.
macro_rules! check_primitive {
    ($module:ident, $raw:ty, $java:ident) => {
        mod $module {
            use abos_jni::{bind_java_type, errors::Result, Env};

            #[repr(transparent)]
            pub struct Wrapped($raw);

            impl From<Wrapped> for $raw {
                fn from(value: Wrapped) -> Self {
                    value.0
                }
            }

            impl From<$raw> for Wrapped {
                fn from(value: $raw) -> Self {
                    Self(value)
                }
            }

            bind_java_type! {
                Binding => "example.PrimitiveMappings",
                type_map = { unsafe Wrapped => $java },
                constructors { fn new(value: Wrapped) },
                methods {
                    fn echo(value: Wrapped) -> Wrapped,
                    static fn echo_static(value: Wrapped) -> Wrapped,
                },
                fields {
                    value: Wrapped,
                    static static_value: Wrapped,
                },
            }

            pub fn exercise(env: &mut Env<'_>) -> Result<()> {
                let object = Binding::new(env, Wrapped(<$raw>::default()))?;
                let value: Wrapped = object.echo(env, object.value(env)?)?;
                object.set_value(env, value)?;
                let value: Wrapped = Binding::echo_static(env, Binding::static_value(env)?)?;
                Binding::set_static_value(env, value)?;
                Ok(())
            }
        }
    };
}

check_primitive!(boolean, u8, boolean);
check_primitive!(byte, i8, byte);
check_primitive!(char, u16, char);
check_primitive!(short, i16, short);
check_primitive!(int, i32, int);
check_primitive!(long, i64, long);
check_primitive!(float, f32, float);
check_primitive!(double, f64, double);

fn main() {
    // Type-check actual API calls without requiring a JVM in this compile-pass fixture.
    let _ = (
        boolean::exercise,
        byte::exercise,
        char::exercise,
        short::exercise,
        int::exercise,
        long::exercise,
        float::exercise,
        double::exercise,
    );
}
