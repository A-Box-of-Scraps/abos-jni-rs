pub const STRING_CASES: [(&str, &str); 4] = [
	("ascii", "abcdefgh"),
	("nul", "abc\0defg"),
	("bmp", "αβγδεζηθ"),
	("non_bmp", "😀😁😂😃😄😅😆😇"),
];
pub const STRING_REPEATS: [usize; 2] = [1, 512];
