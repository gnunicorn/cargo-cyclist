use std::path::{Path, PathBuf};
use std::{fs, io};
use structopt::StructOpt;
use std::collections::HashMap;

#[derive(Debug, StructOpt)]
#[structopt(name = "cargo-cyclist", about = "Detect cycles in your Cargo.lock")]
struct Opt {
	#[structopt(parse(from_os_str))]
	lock_dir: PathBuf,
}

const CARGO_LOCK: &str = "Cargo.lock";

type Package<'a> = (&'a str, &'a str);

fn main() -> Result<(), String> {
	let opt = Opt::from_args();
	let lock_path = if opt.lock_dir.ends_with(CARGO_LOCK) {
		opt.lock_dir.clone()
	} else {
		opt.lock_dir.join(CARGO_LOCK)
	};

	let toml = read_and_parse_toml(&lock_path)
		.map_err(|e| format!("{:?}", e))?;
	let packages = toml.as_table()
		.ok_or_else(|| "Invalid toml file".to_string())?
		.get("package")
		.ok_or_else(|| "No [[package]] section found".to_string())?
		.as_array()
		.ok_or_else(|| "Parsing packages failed".to_string())?;

	let mut package_idx = HashMap::new();

	packages.iter().for_each(|p| {
		if let Some(package) = p.as_table() {
			if let (Some(name), Some(version), Some(deps)) = (
					package.get("name").map(|v| v.as_str()).flatten(),
					package.get("version").map(|v| v.as_str()).flatten(),
					package.get("dependencies").map(|v| v.as_array()).flatten()
			) {
				package_idx.insert( (name, version), deps
					.iter().filter_map(|v| {
						if let Some(dep) = v.as_str() {
							let parts: Vec<&str> = dep.split(" ").collect();
							if parts.len() == 2 {
								// otherwise, the third shows the source. We ignore that
								return Some((parts[0], parts[1]));
							}
						}
						return None
					})
					.collect::<Vec<(&str, &str)>>()
				);
			}
		}
	});
	let mut found = vec![];
	package_idx.iter().for_each(|(package, deps)| {
		check_packages(&package_idx, deps, &mut vec![*package], &mut found);
	});

	Ok(())
}

fn check_packages<'a>(
	idx: &HashMap<Package<'a>, Vec<Package<'a>>>,
	deps: &Vec<Package<'a>>,
	mut cur_path: &mut Vec<Package<'a>>,
	mut found: &mut Vec<Package<'a>>,
) {
	deps.iter().for_each( |cur| {
		if found.contains(cur) {
			// skip already known cycles
			return
		}
		if let Some(pos) = cur_path.iter().position(|x| x == cur) {
			let mut path = cur_path.split_at(pos).1.iter().map(|(x, y)| format!("{:}({:})", x, y)).collect::<Vec<String>>();
			path.push(format!("{:}({:})", cur.0, cur.1));
			println!("{:} ({:}): Cycle through {:}", cur.0, cur.1, path.join(" -> "));
			found.push(*cur);
			return
		} else {
			cur_path.push(*cur);
			idx.get(cur).map(|inner| check_packages(idx, inner, &mut cur_path, &mut found));
			let _ = cur_path.pop();
		}
	});
}

fn read_and_parse_toml(cargo_path: &Path) -> io::Result<toml::Value> {
	use std::io::Read;
	let mut file = fs::File::open(cargo_path)?;
	let mut contents = String::new();
	file.read_to_string(&mut contents)?;
	Ok(toml::from_str(&contents)?)
}