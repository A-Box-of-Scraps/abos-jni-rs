#include <jni.h>
#include <stddef.h>
#include <stdint.h>

#ifndef JNI_VERSION_24
#error "The JNI ABI probe requires JDK 24 or newer headers (set JAVA_HOME)."
#endif

/* Compile-time type checks, derived from the Rust declarations, against jni.h. */
#include "signatures.h"

static const int64_t measurements[] = {
#include "measurements.inc"
};

size_t abos_jni_probe_count(void) {
    return sizeof(measurements) / sizeof(measurements[0]);
}

int64_t abos_jni_probe_value(size_t index) {
    if (index >= abos_jni_probe_count()) {
        return INT64_MIN;
    }
    return measurements[index];
}
