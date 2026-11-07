use super::resolve_name_collision;
use std::collections::HashMap;

#[test]
fn full_member_signatures_and_accessor_roles_have_distinct_names() {
	let mut names: HashMap<String, String> = HashMap::new();
	let members = [
		"Lsample/Example;-><init>(I)V",
		"Lsample/Example;-><init>(J)V",
		"Lsample/Example;->call:I",
		"setter:Lsample/Example;->call:I",
		"Lsample/Example;->call(I)V",
		"Lsample/Example;->call(J)V",
	];
	for (index, member) in members.iter().enumerate() {
		let (name, collision, previous) = resolve_name_collision("call".to_owned(), member, &mut names);
		assert_eq!(name, format!("call{}", "_".repeat(index)));
		assert_eq!(collision, index != 0);
		assert_eq!(previous.as_deref(), (index != 0).then_some(members[0]));
	}
	for (index, member) in members.iter().enumerate() {
		let (name, _, _) = resolve_name_collision("call".to_owned(), member, &mut names);
		assert_eq!(name, format!("call{}", "_".repeat(index)));
	}
	assert_eq!(names.len(), members.len());
}
