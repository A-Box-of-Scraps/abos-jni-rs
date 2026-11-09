"""JVMTI additions after the supported 1.2 baseline.

Source: OpenJDK 25 src/hotspot/share/prims/jvmti.xml, `since` attributes:
https://github.com/openjdk/jdk25u/blob/master/src/hotspot/share/prims/jvmti.xml
JVMTI 19 changed semantics but did not add table slots, events, or capability bits.
"""

import re

FUNCTIONS = {
    "GetAllModules": "9",
    "GetNamedModule": "9",
    "AddModuleReads": "9",
    "AddModuleExports": "9",
    "AddModuleOpens": "9",
    "AddModuleUses": "9",
    "AddModuleProvides": "9",
    "IsModifiableModule": "9",
    "SetHeapSamplingInterval": "11",
    "SuspendAllVirtualThreads": "21",
    "ResumeAllVirtualThreads": "21",
    "ClearAllFramePops": "25",
}
EVENTS = {
    "SampledObjectAlloc": "11",
    "VirtualThreadStart": "21",
    "VirtualThreadEnd": "21",
}
CAPABILITIES = {
    "can_generate_early_vmstart": "9",
    "can_generate_early_class_hook_events": "9",
    "can_generate_sampled_object_alloc_events": "11",
    "can_support_virtual_threads": "21",
}


def constant(version):
    return "JVMTI_VERSION" if version == "25" else "JVMTI_VERSION_" + version.replace(".", "_")


def annotate_table(table):
    names = re.findall(r"pub (\w+):", table)
    reserved = {name for name in names if name.startswith("reserved")}
    if (
        len(names) != 156
        or reserved != {"reserved1", "reserved105", "reserved113", "reserved117", "reserved141"}
        or not FUNCTIONS.keys() <= set(names)
    ):
        raise ValueError("Update JVMTI function-version metadata for this table")
    for name in names:
        if name in FUNCTIONS or name.startswith("reserved"):
            version = FUNCTIONS.get(name, "reserved")
            table = table.replace(f"pub {name}:", f'#[jvmti_added("{version}")]\npub {name}:')
    return "#[abos_jni_sys_macros::jvmti_versions]\n" + table


def compatibility(event_table, capability_names):
    events = re.findall(r"pub (\w+):", event_table)
    if (
        len(capability_names) != 45
        or len(events) != 39
        or not EVENTS.keys() <= set(events)
        or not CAPABILITIES.keys() <= set(capability_names)
    ):
        raise ValueError("Update JVMTI event/capability version metadata")
    lines = ["use super::*;", "impl jvmtiCapabilities {"]
    lines.extend(required_version(CAPABILITIES, "()"))
    lines.extend(["}", "impl jvmtiEventCallbacks {"])
    lines.extend(required_version(EVENTS, ".is_some()"))
    lines.extend(
        [
            "/// Size of the known callback prefix for a valid JVMTI version.",
            "#[must_use]",
            "pub const fn size_for_version(version: jint) -> Option<usize> {",
        ]
    )
    for version, last in [("21", "VirtualThreadEnd"), ("11", "SampledObjectAlloc"), ("1.2", "VMObjectAlloc")]:
        lines.append(
            f"if version >= {constant(version)} {{ return Some(core::mem::offset_of!(Self, {last}) + core::mem::size_of::<jvmtiEvent{last}>()); }}"
        )
    lines.extend(
        [
            "None",
            "}",
            "}",
            "/// Minimum supported wrapper version for a standard event; reserved IDs return None.",
            "#[must_use]",
            "pub const fn jvmti_event_version(event: jvmtiEvent) -> Option<jint> {",
            "match event {",
        ]
    )
    groups = {}
    for name in events:
        if not name.startswith("reserved"):
            words = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
            words = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", words)
            groups.setdefault(EVENTS.get(name, "1.2"), []).append("JVMTI_EVENT_" + words.upper())
    for version, names in groups.items():
        lines.append(f"{' | '.join(names)} => Some({constant(version)}),")
    lines.extend(["_ => None,", "}", "}"])
    return "\n".join(lines)


def required_version(entries, accessor):
    groups = {}
    for name, version in entries.items():
        groups.setdefault(int(version), []).append(f"self.{name}{accessor}")
    lines = [
        "/// Minimum supported wrapper version for the selected entries.",
        "#[must_use]",
        "pub const fn required_version(&self) -> jint {",
    ]
    for version in sorted(groups, reverse=True):
        lines.append(f"if {' || '.join(groups[version])} {{ return {constant(str(version))}; }}")
    lines.extend(["JVMTI_VERSION_1_2", "}"])
    return lines
