use super::Subject;

#[test]
fn private_access() {
    let _ = Subject;
}

#[cfg(test)]
const _: () = assert!(true);
