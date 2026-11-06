//! Execute self, cross-class, array, and inherited default-package references.
use crate::bindings::{DefaultPeer, DefaultType, DefaultTypeChild};
use abos_jni::{Env, errors::Result, jni_str, objects::JObject, refs::Reference};

pub fn check(env: &mut Env<'_>) -> Result<()> {
	let instance = DefaultType::new(env)?;
	let echoed: DefaultType<'_> = instance.echo(env, &instance)?;
	assert!(env.is_same_object(&instance, &echoed)?);
	let peer = DefaultPeer::new(env, &instance)?;
	let owner: DefaultType<'_> = peer.echo_owner(env, &instance)?;
	assert!(env.is_same_object(&instance, &owner)?);
	instance.set_peer(env, &peer)?;
	let read: DefaultPeer<'_> = instance.peer(env)?;
	assert!(env.is_same_object(&peer, &read)?);
	let echoed_peer = instance.echo_peer(env, &peer)?;
	assert!(env.is_same_object(&peer, &echoed_peer)?);

	let peers = abos_jni::objects::JObjectArray::<DefaultPeer>::new(env, 1, &peer)?;
	let echoed_peers = instance.echo_peers(env, &peers)?;
	assert!(env.is_same_object(&peers, &echoed_peers)?);
	let matrix = env.new_object_array(0, jni_str!("[LDefaultType;"), JObject::null())?;
	let matrix = env
		.new_cast_local_ref::<abos_jni::objects::JObjectArray<abos_jni::objects::JObjectArray<DefaultType>>>(&matrix)?;
	let echoed_matrix = instance.echo_matrix(env, &matrix)?;
	assert!(env.is_same_object(&matrix, &echoed_matrix)?);
	let child = DefaultTypeChild::new(env)?;
	let echoed_child: DefaultTypeChild<'_> = instance.echo_child(env, &child)?;
	assert!(env.is_same_object(&child, &echoed_child)?);
	let parent: &DefaultType<'_> = child.as_ref();
	let returned_parent = child.parent(env)?;
	assert!(env.is_same_object(parent, &returned_parent)?);

	let from_bytecode = crate::default_bytecode::DefaultType::new(env)?;
	let returned: DefaultPeer<'_> = from_bytecode.echo_peer(env, &peer)?;
	assert!(env.is_same_object(&peer, &returned)?);
	let unmapped = crate::default_unmapped::DefaultType::new(env)?;
	let returned_child: JObject<'_> = unmapped.echo_child(env, &child)?;
	assert!(env.is_same_object(&child, &returned_child)?);
	let returned: JObject<'_> = unmapped.echo_peer(env, &peer)?;
	assert!(env.is_same_object(&peer, &returned)?);
	unmapped.set_peer(env, &peer)?;
	assert!(!unmapped.peer(env)?.is_null());
	Ok(())
}
