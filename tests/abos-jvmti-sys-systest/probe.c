#include <jvmti.h>
#include <stddef.h>
#include <stdint.h>

_Static_assert(JVMTI_VERSION == 0x30190000, "The probe requires JDK 25 headers");

/* Derive expected C signatures from the Rust declarations, not from jvmti.h. */
#include "signatures.h"

static const int64_t measurements[] = {
#include "measurements.inc"
};

size_t abos_jvmti_probe_count(void) {
    return sizeof(measurements) / sizeof(measurements[0]);
}

int64_t abos_jvmti_probe_value(size_t index) {
    return index < abos_jvmti_probe_count() ? measurements[index] : INT64_MIN;
}

unsigned int abos_jvmti_get_capability(const jvmtiCapabilities *caps, size_t index) {
    switch (index) {
#include "getters.inc"
    default: return UINT32_MAX;
    }
}

void abos_jvmti_set_capability(jvmtiCapabilities *caps, size_t index, unsigned int value) {
    switch (index) {
#include "setters.inc"
    default: break;
    }
}
