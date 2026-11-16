// cargo generate: the new plugin's Cargo.toml is Cargo.toml.tmpl, with your crate's name in it.
file::delete("Cargo.toml");
file::rename("Cargo.toml.tmpl", "Cargo.toml");
file::delete("name.rhai");
