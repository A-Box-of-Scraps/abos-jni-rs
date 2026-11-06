use crate::{AttachGuard, ScopeToken};
static_assertions::assert_not_impl_any!(ScopeToken: Send);
static_assertions::assert_not_impl_any!(ScopeToken: Sync);

static_assertions::assert_not_impl_any!(AttachGuard: Send);
static_assertions::assert_not_impl_any!(AttachGuard: Sync);
