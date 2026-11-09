use abos_jni_sys_macros::jvmti_versions;

#[jvmti_versions]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Table {
    pub baseline: Option<unsafe extern "system" fn()>,
    #[jvmti_added("9")]
    pub modules: Option<unsafe extern "system" fn()>,
    pub existing: Option<unsafe extern "system" fn()>,
    #[jvmti_added("11")]
    pub sampling: Option<unsafe extern "system" fn()>,
}

fn main() {
    let baseline = Table_1_2::default();
    let _ = baseline.modules;
    let _ = baseline.sampling;
}
