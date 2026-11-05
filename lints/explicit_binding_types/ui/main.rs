#![allow(dead_code, unused_variables)]

struct ServerPlayer;
impl ServerPlayer {
	fn new() -> Self {
		Self
	}
	fn get_player(&self) -> Self {
		Self
	}
}
fn get_player() -> ServerPlayer {
	ServerPlayer
}
fn players() -> Vec<ServerPlayer> {
	Vec::new()
}
fn opaque() -> impl Iterator<Item = i32> {
	0..3
}
struct Misleading;
impl Misleading {
	fn new() -> ServerPlayer {
		ServerPlayer
	}
}
macro_rules! generated {
	() => {
		let generated_player = get_player();
	};
}

fn main() {
	// Obvious or explicitly annotated bindings.
	let name = "toto";
	let enabled = true;
	let count = 42;
	let negative = -42;
	let player = ServerPlayer::new();
	let player = ServerPlayer {};
	let list = Vec::<ServerPlayer>::new();
	let player: ServerPlayer = get_player();
	let list: Vec<ServerPlayer> = players();
	let pair = ("name", true);
	let array = [1, 2];
	let borrowed = &"name";
	let cast = count as u64;
	let closure = || get_player();
	let iterator = opaque();
	generated!();
	let _ = get_player();
	let (first, second) = (get_player(), get_player());
	#[allow(explicit_binding_types)]
	let allowed = get_player();

	// Non-obvious initializers.
	let player = get_player();
	let other = player.get_player();
	let mut list = Vec::new();
	list.push(ServerPlayer);
	let inferred: _ = get_player();
	let partial: Vec<_> = players();
	let nested: Vec<Option<_>> = std::iter::once(Some(get_player())).collect();
	let mut partial_constructor = Vec::<_>::new();
	partial_constructor.push(ServerPlayer);
	let misleading = Misleading::new();
	let pair = (get_player(), true);
	let copied = player;
}
