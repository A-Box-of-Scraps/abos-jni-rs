import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "generate-jvmti.py"
SPEC = importlib.util.spec_from_file_location("generate_jvmti", SCRIPT)
GENERATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GENERATOR)


class JvmtiGeneratorTests(unittest.TestCase):
    def test_pointer_constness_and_shared_jni_types(self):
        self.assertEqual(GENERATOR.rust_type("const unsigned char**"), "*mut *const u8")
        self.assertEqual(GENERATOR.rust_type("JNIEnv*"), "*mut JNIEnv")
        self.assertEqual(GENERATOR.rust_type("jobject"), "jobject")
        self.assertEqual(GENERATOR.rust_type("struct jvmtiInterface_1_*"), "*mut jvmtiInterface_1_")

    def test_callback_abi_and_nullable_representation(self):
        self.assertEqual(
            GENERATOR.function("void", "jvmtiEnv* env, jint count"),
            'Option<unsafe extern "system" fn(env: *mut jvmtiEnv, count: jint)>',
        )
        self.assertEqual(
            GENERATOR.function("jvmtiError", "jvmtiEnv* env, ...", named=False),
            'Option<unsafe extern "C" fn(*mut jvmtiEnv, ...) -> jvmtiError>',
        )

    def test_records_preserve_reserved_slots_and_aliases(self):
        records = dict(GENERATOR.records("typedef struct table { void* reserved; jint (JNICALL *Get)(void); } alias;"))
        self.assertIn("pub reserved: *mut c_void,", records["table"])
        self.assertIn('pub Get: Option<unsafe extern "system" fn() -> jint>', records["table"])
        self.assertIn("pub type alias = table;", records["table"])

    def test_missing_capabilities_declaration_reports_a_clear_error(self):
        header = "enum { JVMTI_VERSION = 0x30000000 + (25 * 0x10000) };"
        with self.assertRaisesRegex(ValueError, "Missing or unsupported jvmtiCapabilities declaration"):
            GENERATOR.generate(header)

    def test_unknown_declarations_fail(self):
        for field in ["int flags: 3", "jint values[4]"]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                GENERATOR.field(field)
        with self.assertRaises(ValueError):
            GENERATOR.rust_type("jint* const *")
        with self.assertRaisesRegex(ValueError, "JVMTI 25"):
            GENERATOR.generate("enum { JVMTI_VERSION = 0x30000000 + (26 * 0x10000) }; ")


if __name__ == "__main__":
    unittest.main()
