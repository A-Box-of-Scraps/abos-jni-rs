#[path = "auxiliary/at_limit.rs"]
mod at_limit;
#[path = "auxiliary/too_long.rs"]
mod too_long;
// The same physical file should only produce one warning.
#[path = "auxiliary/too_long.rs"]
mod repeated;
#[path = "auxiliary/empty.rs"]
mod empty;
#[path = "auxiliary/blank.rs"]
mod blank;
#[allow(file_length)]
#[path = "auxiliary/allowed.rs"]
mod allowed;

#[path = "auxiliary/inner_allowed.rs"]
mod inner_allowed;

include!("auxiliary/included.rs");
include!("auxiliary/target/generated.rs");
#[path = "auxiliary/.hidden.rs"]
mod hidden;

mod inline {
    // An inline module is not a separate physical file.
    pub fn example() {}
}

fn main() {
    inline::example();
}
