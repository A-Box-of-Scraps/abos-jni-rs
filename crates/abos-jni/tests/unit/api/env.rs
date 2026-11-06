use crate::Env;
static_assertions::assert_not_impl_any!(Env: Send);
static_assertions::assert_not_impl_any!(Env: Sync);

static_assertions::assert_not_impl_any!(super::MonitorGuard: Send);
