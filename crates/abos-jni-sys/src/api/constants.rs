use super::{jboolean, jint};

pub const JNI_FALSE: jboolean = 0;
pub const JNI_TRUE: jboolean = 1;

pub const JNI_OK: jint = 0;
pub const JNI_ERR: jint = -1;
pub const JNI_EDETACHED: jint = -2;
pub const JNI_EVERSION: jint = -3;
pub const JNI_ENOMEM: jint = -4;
pub const JNI_EEXIST: jint = -5;
pub const JNI_EINVAL: jint = -6;

pub const JNI_COMMIT: jint = 1;
pub const JNI_ABORT: jint = 2;

pub const JNI_VERSION_1_1: jint = 0x0001_0001;
pub const JNI_VERSION_1_2: jint = 0x0001_0002;
pub const JNI_VERSION_1_4: jint = 0x0001_0004;
pub const JNI_VERSION_1_6: jint = 0x0001_0006;
pub const JNI_VERSION_1_8: jint = 0x0001_0008;
pub const JNI_VERSION_9: jint = 0x0009_0000;
pub const JNI_VERSION_10: jint = 0x000a_0000;
pub const JNI_VERSION_19: jint = 0x0013_0000;
pub const JNI_VERSION_20: jint = 0x0014_0000;
pub const JNI_VERSION_21: jint = 0x0015_0000;
pub const JNI_VERSION_24: jint = 0x0018_0000;
