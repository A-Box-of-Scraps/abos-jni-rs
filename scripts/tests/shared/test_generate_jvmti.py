import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / "generate-jvmti.py"
SPEC = importlib.util.spec_from_file_location("generate_jvmti", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise ImportError(f"Cannot load generator module from {SCRIPT}")
GENERATOR = importlib.util.module_from_spec(SPEC)
with patch.object(sys, "path", [str(SCRIPT.parent), *sys.path]):
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

    def test_enum_aliases_follow_the_target_c_abi_not_the_generator_host(self):
        self.assertEqual(
            GENERATOR.enum_alias("Kind", False),
            '#[cfg(target_env = "msvc")]\npub type Kind = i32;\n#[cfg(not(target_env = "msvc"))]\npub type Kind = u32;',
        )
        self.assertEqual(GENERATOR.enum_alias("SignedKind", True), "pub type SignedKind = i32;")

    def test_records_preserve_reserved_slots_and_aliases(self):
        records = dict(GENERATOR.records("typedef struct table { void* reserved; jint (JNICALL *Get)(void); } alias;"))
        self.assertIn("pub reserved: *mut c_void,", records["table"])
        self.assertIn('pub Get: Option<unsafe extern "system" fn() -> jint>', records["table"])
        self.assertIn("pub type alias = table;", records["table"])

    def test_missing_capabilities_declaration_reports_a_clear_error(self):
        header = "enum { JVMTI_VERSION = 0x30000000 + (25 * 0x10000) };"
        with self.assertRaisesRegex(ValueError, "Missing or unsupported jvmtiCapabilities declaration"):
            GENERATOR.generate(header)

    def test_version_annotations_cover_new_functions_and_reject_new_slots(self):
        table = (SCRIPT.parent.parent / "crates/abos-jvmti-sys/src/api/interface.rs").read_text()
        annotated = GENERATOR.jvmti_versions.annotate_table(table)
        for name, version in [
            ("GetAllModules", "9"),
            ("SetHeapSamplingInterval", "11"),
            ("SuspendAllVirtualThreads", "21"),
            ("ClearAllFramePops", "25"),
        ]:
            self.assertIn(f'#[jvmti_added("{version}")]\npub {name}:', annotated)
        with self.assertRaisesRegex(ValueError, "function-version metadata"):
            GENERATOR.jvmti_versions.annotate_table(table.replace("reserved105", "NewFunction"))

    def test_table_annotation_preserves_other_fields_and_text(self):
        versions = GENERATOR.jvmti_versions
        reserved = ["reserved1", "reserved105", "reserved113", "reserved117", "reserved141"]
        names = [*reserved, *versions.FUNCTIONS]
        names.extend(f"Baseline{index}" for index in range(156 - len(names)))
        fields = [f"pub {name}: jint," for name in names]
        table = "pub struct Table {\n" + "\n".join(fields) + "\n}"
        expected_fields = []
        for name, field in zip(names, fields):
            if name in reserved or name in versions.FUNCTIONS:
                version = versions.FUNCTIONS.get(name, "reserved")
                field = f'#[jvmti_added("{version}")]\n{field}'
            expected_fields.append(field)
        expected = "#[abos_jni_sys_macros::jvmti_versions]\npub struct Table {\n" + "\n".join(expected_fields) + "\n}"
        self.assertEqual(versions.annotate_table(table), expected)

    def test_capability_versions_are_checked_newest_first(self):
        output = "\n".join(GENERATOR.jvmti_versions.required_version(GENERATOR.jvmti_versions.CAPABILITIES, "()"))
        self.assertLess(output.index("JVMTI_VERSION_21"), output.index("JVMTI_VERSION_11"))
        self.assertLess(output.index("JVMTI_VERSION_11"), output.index("JVMTI_VERSION_9"))
        self.assertIn("self.can_generate_early_vmstart() || self.can_generate_early_class_hook_events()", output)
        self.assertTrue(output.endswith("JVMTI_VERSION_1_2\n}"))

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
