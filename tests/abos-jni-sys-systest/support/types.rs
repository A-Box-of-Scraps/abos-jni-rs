use syn::{GenericArgument, PathArguments, PointerMutability, ReturnType, Type, TypeFnPtr};

pub fn c_name(name: &str) -> String {
	match name {
		"JNINativeInterface_"
		| "JNIInvokeInterface_"
		| "JNIEnv_"
		| "_jobject"
		| "_jfieldID"
		| "_jmethodID"
		| "jvmtiInterface_1_"
		| "_jvmtiEnv"
		| "_jrawMonitorID" => {
			format!("struct {name}")
		}
		"c_void" => "void".to_owned(),
		"c_char" => "char".to_owned(),
		"i8" => "int8_t".to_owned(),
		"u8" => "uint8_t".to_owned(),
		"i16" => "int16_t".to_owned(),
		"u16" => "uint16_t".to_owned(),
		"i32" => "int32_t".to_owned(),
		"u32" => "uint32_t".to_owned(),
		"i64" => "int64_t".to_owned(),
		"u64" => "uint64_t".to_owned(),
		"f32" => "float".to_owned(),
		"f64" => "double".to_owned(),
		"bool" => "_Bool".to_owned(),
		other => other.to_owned(),
	}
}

pub fn c_type(ty: &Type) -> String {
	match ty {
		Type::Path(path)
			if path.qself.is_none()
				&& path.path.segments.len() == 3
				&& path.path.segments[0].ident == "core"
				&& path.path.segments[1].ident == "ffi"
				&& path.path.segments[2].ident == "VaList" =>
		{
			assert_lifetime_arguments(&path.path.segments[2].arguments);
			"va_list".to_owned()
		}
		Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
			let segment = &path.path.segments[0];
			if segment.ident == "Option" {
				let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
					panic!("Option requires a function pointer");
				};
				assert_eq!(arguments.args.len(), 1, "Option requires one argument");
				let Some(GenericArgument::Type(Type::FnPtr(function))) = arguments.args.first() else {
					panic!("Only nullable function pointers have a C representation");
				};
				return function_type(function);
			}
			if segment.ident == "va_list" {
				assert_lifetime_arguments(&segment.arguments);
			} else {
				assert!(segment.arguments.is_empty(), "Generic C types are unsupported");
			}
			c_name(&segment.ident.to_string())
		}
		Type::Ptr(pointer) => {
			let element = c_type(&pointer.elem);
			let qualifier = if matches!(pointer.mutability, PointerMutability::Const(_)) {
				" const"
			} else {
				""
			};
			format!("{element}{qualifier} *")
		}
		Type::Array(array) => format!("{} *", c_type(&array.elem)),
		Type::FnPtr(function) => function_type(function),
		// C's FatalError returns void; Rust additionally models its non-returning behavior.
		Type::Never(_) => "void".to_owned(),
		_ => panic!("Unsupported JNI type: {}", quote::quote!(#ty)),
	}
}

pub fn function_type(function: &TypeFnPtr) -> String {
	assert!(
		function.lifetimes.is_none(),
		"Generic function pointers are unsupported"
	);
	let abi = function
		.abi
		.as_ref()
		.and_then(|abi| abi.name.as_ref())
		.expect("Explicit C or system ABI")
		.value();
	let convention = match abi.as_str() {
		"system" => "JNICALL ",
		"C" => "",
		_ => panic!("Unsupported JNI calling convention: {abi}"),
	};
	let result = match &function.output {
		ReturnType::Default => "void".to_owned(),
		ReturnType::Type(_, ty) => c_type(ty),
	};
	let mut parameters: Vec<String> = function.inputs.iter().map(|argument| c_type(&argument.ty)).collect();
	if function.variadic.is_some() {
		parameters.push("...".to_owned());
	}
	if parameters.is_empty() {
		parameters.push("void".to_owned());
	}
	format!("{result} ({convention}*)({})", parameters.join(", "))
}

fn assert_lifetime_arguments(arguments: &PathArguments) {
	assert!(
		matches!(arguments, PathArguments::AngleBracketed(args)
		if args.args.len() == 1 && matches!(args.args.first(), Some(GenericArgument::Lifetime(_)))),
		"va_list requires exactly one lifetime argument"
	);
}
