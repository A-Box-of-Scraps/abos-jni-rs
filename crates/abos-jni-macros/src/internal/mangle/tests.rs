use super::*;

#[test]
fn test_create_jni_fn_name() {
	// Basic namespace and function name tests
	assert_eq!(
		create_jni_fn_name("com.example.Foo", "init", None),
		"Java_com_example_Foo_init"
	);
	assert_eq!(
		create_jni_fn_name("com.example.Bar", "closeIt", None),
		"Java_com_example_Bar_closeIt"
	);
	assert_eq!(
		create_jni_fn_name("com.example.Bar", "close_it", None),
		"Java_com_example_Bar_close_1it"
	);
	assert_eq!(
		create_jni_fn_name("org.signal.client.internal.Native", "IdentityKeyPair_Deserialize", None),
		"Java_org_signal_client_internal_Native_IdentityKeyPair_1Deserialize"
	);
	assert_eq!(
		create_jni_fn_name("a.b.c.Test$", "show", None),
		"Java_a_b_c_Test_00024_show"
	);

	// Tests with signatures - only argument types are encoded, no parens or return type
	assert_eq!(
		create_jni_fn_name("com.example.Foo", "method", Some("(I)Z")),
		"Java_com_example_Foo_method__I"
	);
	assert_eq!(
		create_jni_fn_name("com.example.Bar", "test", Some("(Ljava/lang/String;)V")),
		"Java_com_example_Bar_test__Ljava_lang_String_2"
	);
	assert_eq!(
		create_jni_fn_name("a.b.Test", "arrayMethod", Some("([I)[Ljava/lang/Object;")),
		"Java_a_b_Test_arrayMethod___3I"
	);
	assert_eq!(
		create_jni_fn_name("com.example.Test", "complex_method", Some("([[Ljava/lang/String;I)[[I")),
		"Java_com_example_Test_complex_1method___3_3Ljava_lang_String_2I"
	);
	// Test with no arguments (empty parentheses) - should still have __ suffix
	assert_eq!(
		create_jni_fn_name("com.example.Foo", "noArgs", Some("()V")),
		"Java_com_example_Foo_noArgs__"
	);
}

#[test]
fn test_valid_namespace() {
	// Valid namespaces
	assert!(valid_namespace("com.example.Foo"));
	assert!(valid_namespace("com.antonok.kb"));
	assert!(valid_namespace("org.signal.client.internal.Native"));
	assert!(valid_namespace("net.under_score"));
	assert!(valid_namespace("a.b.c.Test$"));

	// Invalid namespaces - spaces and special characters
	assert!(!valid_namespace("com example Foo"));
	assert!(!valid_namespace(" com.example.Foo"));
	assert!(!valid_namespace("com.example.Foo "));
	assert!(!valid_namespace("com.example.1Foo"));

	// Invalid namespaces - leading dots
	assert!(!valid_namespace(".com.example.Foo"));
	assert!(!valid_namespace("."));

	// Invalid namespaces - trailing dots
	assert!(!valid_namespace("com.example.Foo."));

	// Invalid namespaces - consecutive dots
	assert!(!valid_namespace("com..example.Foo"));
	assert!(!valid_namespace("com...example.Foo"));
	assert!(!valid_namespace("com.example..Foo"));
}

#[test]
fn test_code_generation() {
	let attr = quote::quote! {
		"com.example.Bar"
	};
	let source = quote::quote! {
		pub fn close_it(env: JNIEnv, _: JClass, filename: JString) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	// Note: close_it becomes closeIt (lowerCamelCase)
	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Bar_closeIt")]
				#[allow(non_snake_case)]
				pub extern "system" fn close_it(env: JNIEnv, _: JClass, filename: JString) -> jboolean {
					unimplemented!()
				}
			}
		)
	);
}

#[test]
fn test_unsafe_fn() {
	let attr = quote::quote! {
		"com.example.Bar"
	};
	let source = quote::quote! {
		pub unsafe fn close_it(env: JNIEnv, _: JClass, filename: JString) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	// Note: close_it becomes closeIt (lowerCamelCase)
	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Bar_closeIt")]
				#[allow(non_snake_case)]
				pub unsafe extern "system" fn close_it(env: JNIEnv, _: JClass, filename: JString) -> jboolean {
					unimplemented!()
				}
			}
		)
	);
}

#[test]
fn test_non_function() {
	let attr = quote::quote! { "com.example.Foo" };
	let source = quote::quote! {
		enum NotAFunction {
			Variant1,
			Variant2(u8),
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				::core::compile_error! { "The `jni_mangle` attribute can only be applied to `fn` items" }
			}
		)
	);
}

#[test]
fn test_empty_attribute() {
	let attr = quote::quote! {};
	let source = quote::quote! {
		pub fn close_it(env: JNIEnv, _: JClass, filename: JString) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				::core::compile_error! { "The `jni_mangle` attribute must have 1-3 string literal arguments" }
			}
		)
	);
}

#[test]
fn test_invalid_namespace() {
	let attr = quote::quote! { "." };
	let source = quote::quote! {
		pub fn close_it(env: JNIEnv, _: JClass, filename: JString) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				::core::compile_error! { "Invalid package namespace supplied to `jni_mangle` attribute" }
			}
		)
	);
}

#[test]
fn test_wrong_abi_generates_error() {
	let attr = quote::quote! { "com.example.Foo" };
	let source = quote::quote! {
		pub extern "C" fn closeIt(env: JNIEnv, _: JClass, filename: JString) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	// Should generate an error for non-system ABI
	let expanded_str = format!("{expanded}");
	assert!(expanded_str.contains("compile_error"));
	assert!(expanded_str.contains("must use `extern \\\"system\\\"` ABI"));
	assert!(expanded_str.contains("found `extern \\\"C\\\"`"));
}

#[test]
fn test_system_abi_is_preserved() {
	let attr = quote::quote! { "com.example.Foo" };
	let source = quote::quote! {
		pub extern "system" fn closeIt(env: JNIEnv, _: JClass, filename: JString) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	// The "system" ABI should be preserved
	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Foo_closeIt")]
				#[allow(non_snake_case)]
				pub extern "system" fn closeIt (env: JNIEnv, _: JClass, filename: JString) -> jboolean {
					unimplemented!()
				}
			}
		)
	);
}

#[test]
fn test_wrong_visibility() {
	let attr = quote::quote! { "com.example.Foo" };
	let source = quote::quote! {
		fn close_it(env: JNIEnv, _: JClass, filename: JString) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				::core::compile_error! { "`jni_mangle` attributed functions must have public visibility (`pub`)" }
			}
		)
	);
}

#[test]
fn test_with_method_name() {
	let attr = quote::quote! {
		"com.example.Bar", "customMethod"
	};
	let source = quote::quote! {
		pub fn rust_function(env: JNIEnv, _: JClass) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Bar_customMethod")]
				#[allow(non_snake_case)]
				pub extern "system" fn rust_function (env: JNIEnv, _: JClass) -> jboolean {
					unimplemented!()
				}
			}
		)
	);
}

#[test]
fn test_with_signature_only() {
	let attr = quote::quote! {
		"com.example.Bar", "(I)Z"
	};
	let source = quote::quote! {
		pub fn boolMethod(env: JNIEnv, _: JClass) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Bar_boolMethod__I")]
				#[allow(non_snake_case)]
				pub extern "system" fn boolMethod (env: JNIEnv, _: JClass) -> jboolean {
					unimplemented!()
				}
			}
		)
	);
}

#[test]
fn test_with_method_name_and_signature() {
	let attr = quote::quote! {
		"com.example.Bar", "testMethod", "(Ljava/lang/String;)V"
	};
	let source = quote::quote! {
		pub fn rust_func(env: JNIEnv, _: JClass) {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Bar_testMethod__Ljava_lang_String_2")]
				#[allow(non_snake_case)]
				pub extern "system" fn rust_func (env: JNIEnv, _: JClass) {
					unimplemented!()
				}
			}
		)
	);
}

#[test]
fn test_snake_case_to_lower_camel_case() {
	// Basic conversions
	assert_eq!(snake_case_to_lower_camel_case("say_hello"), "sayHello");
	assert_eq!(snake_case_to_lower_camel_case("get_user_name"), "getUserName");
	assert_eq!(snake_case_to_lower_camel_case("init"), "init");
	assert_eq!(snake_case_to_lower_camel_case("close_it"), "closeIt");

	// Idempotent - already lowerCamelCase
	assert_eq!(snake_case_to_lower_camel_case("sayHello"), "sayHello");
	assert_eq!(snake_case_to_lower_camel_case("getUserName"), "getUserName");
	assert_eq!(snake_case_to_lower_camel_case("closeIt"), "closeIt");

	// Mixed case - preserved unchanged
	assert_eq!(snake_case_to_lower_camel_case("Foo_Bar"), "Foo_Bar");
	assert_eq!(snake_case_to_lower_camel_case("XMLParser"), "XMLParser");
	assert_eq!(snake_case_to_lower_camel_case("IOError"), "IOError");
	assert_eq!(snake_case_to_lower_camel_case("HTML_Parser"), "HTML_Parser");

	// Unicode support
	assert_eq!(snake_case_to_lower_camel_case("café_résumé"), "caféRésumé");
	assert_eq!(snake_case_to_lower_camel_case("ß_test"), "ßTest");
	assert_eq!(snake_case_to_lower_camel_case("test_αλφα"), "testΑλφα");
	assert_eq!(snake_case_to_lower_camel_case("method_привет"), "methodПривет");

	// Leading underscores - one is removed
	assert_eq!(snake_case_to_lower_camel_case("_private_method"), "privateMethod");
	assert_eq!(snake_case_to_lower_camel_case("__dunder__"), "_dunder__");
	assert_eq!(snake_case_to_lower_camel_case("_leading"), "leading");
	assert_eq!(snake_case_to_lower_camel_case("__leading_multiple"), "_leadingMultiple");
	assert_eq!(snake_case_to_lower_camel_case("___priv"), "__priv");
	assert_eq!(snake_case_to_lower_camel_case("_priv_name"), "privName");
	assert_eq!(snake_case_to_lower_camel_case("__priv_name"), "_privName");

	// Trailing underscores preserved
	assert_eq!(snake_case_to_lower_camel_case("trailing_"), "trailing_");
	assert_eq!(
		snake_case_to_lower_camel_case("trailing_multiple__"),
		"trailingMultiple__"
	);
	assert_eq!(snake_case_to_lower_camel_case("_a_"), "a_");
	assert_eq!(snake_case_to_lower_camel_case("_foo_bar_"), "fooBar_");

	// All underscores
	assert_eq!(snake_case_to_lower_camel_case("___"), "___");

	// Edge cases
	assert_eq!(snake_case_to_lower_camel_case("a_b_c"), "aBC");

	// Numeric prefixes - capitalize first non-digit character
	assert_eq!(snake_case_to_lower_camel_case("array_2d_foo"), "array2DFoo");
	assert_eq!(snake_case_to_lower_camel_case("test_3d"), "test3D");
	assert_eq!(snake_case_to_lower_camel_case("foo_123bar"), "foo123Bar");
	assert_eq!(snake_case_to_lower_camel_case("get_2d_array"), "get2DArray");
	assert_eq!(snake_case_to_lower_camel_case("test_42"), "test42");
	assert_eq!(snake_case_to_lower_camel_case("a_1_b"), "a1B");
}

#[test]
fn test_snake_case_function_name_conversion() {
	// Test that snake_case function names are automatically converted to lowerCamelCase
	let attr = quote::quote! {
		"com.example.Bar"
	};
	let source = quote::quote! {
		pub fn say_hello_world(env: JNIEnv, _: JClass) {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Bar_sayHelloWorld")]
				#[allow(non_snake_case)]
				pub extern "system" fn say_hello_world (env: JNIEnv, _: JClass) {
					unimplemented!()
				}
			}
		)
	);
}

#[test]
fn test_camel_case_function_name_unchanged() {
	// Test that lowerCamelCase function names remain unchanged (idempotent)
	let attr = quote::quote! {
		"com.example.Bar"
	};
	let source = quote::quote! {
		pub fn sayHelloWorld(env: JNIEnv, _: JClass) {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Bar_sayHelloWorld")]
				#[allow(non_snake_case)]
				pub extern "system" fn sayHelloWorld (env: JNIEnv, _: JClass) {
					unimplemented!()
				}
			}
		)
	);
}

#[test]
fn test_snake_case_with_signature() {
	// Test that snake_case function names are converted even when signature is present
	let attr = quote::quote! {
		"com.example.Bar", "(I)Z"
	};
	let source = quote::quote! {
		pub fn check_valid(env: JNIEnv, _: JClass) -> jboolean {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Bar_checkValid__I")]
				#[allow(non_snake_case)]
				pub extern "system" fn check_valid (env: JNIEnv, _: JClass) -> jboolean {
					unimplemented!()
				}
			}
		)
	);
}

#[test]
fn test_signature_with_no_args() {
	// Test that signatures with no arguments still add __ suffix (for overloaded methods)
	let attr = quote::quote! {
		"com.example.Bar", "()V"
	};
	let source = quote::quote! {
		pub fn noArgs(env: JNIEnv, _: JClass) {
			unimplemented!()
		}
	};

	let expanded = jni_mangle2(attr, source);

	// Should have __ even with no arguments (indicates overloaded method)
	assert_eq!(
		format!("{expanded}"),
		format!(
			"{}",
			quote::quote! {
				#[unsafe(export_name = "Java_com_example_Bar_noArgs__")]
				#[allow(non_snake_case)]
				pub extern "system" fn noArgs (env: JNIEnv, _: JClass) {
					unimplemented!()
				}
			}
		)
	);
}
