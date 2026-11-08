#include <jni.h>
#include <stdarg.h>

/* Exercise real argument passing, not just declarations translated back to C. */
jboolean abos_jni_probe_boolean(jboolean value, jboolean *copy, jvalue *slot,
                                jboolean (JNICALL *callback)(jboolean)) {
    *copy = value;
    slot->z = value;
    return callback(value);
}

static jdouble JNICALL sum_arguments(JNIEnv *env, jobject object, jmethodID method, va_list args) {
    (void)env;
    (void)object;
    (void)method;
    jdouble sum = 0;
    /* Twelve integer/double pairs exhaust both register banks on this host. */
    for (int i = 0; i < 12; i++) {
        sum += va_arg(args, int);
        sum += va_arg(args, double);
    }
    return sum;
}

static const struct JNINativeInterface_ table = {
    .CallDoubleMethodV = sum_arguments,
};

const struct JNINativeInterface_ *abos_jni_probe_table(void) {
    return &table;
}

typedef jdouble (JNICALL *method_v)(JNIEnv *, jobject, jmethodID, va_list);

static jdouble call_with_list(method_v callback, ...) {
    va_list args;
    va_start(args, callback);
    jdouble result = callback(NULL, NULL, NULL, args);
    va_end(args);
    return result;
}

jdouble abos_jni_probe_call_rust(method_v callback) {
    return call_with_list(callback,
                         1, 0.5, 2, 1.5, 3, 2.5, 4, 3.5,
                         5, 4.5, 6, 5.5, 7, 6.5, 8, 7.5,
                         9, 8.5, 10, 9.5, 11, 10.5, 12, 11.5);
}
