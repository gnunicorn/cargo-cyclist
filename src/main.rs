use std::path::{Path, PathBuf};
use std::{fs, io};
use structopt::StructOpt;
use std::collections::HashMap;

#[derive(Debug, StructOpt)]
#[structopt(name = "cargo-cyclist", about = "Detect cycles in your Cargo.lock")]
struct Opt {
	#[structopt(parse(from_os_str))]
	lock_dir: PathBuf,
	#[structopt(long, short)]
	/// Display result in github flawored markdown
	github: bool

}

const CARGO_LOCK: &str = "Cargo.lock";

type Package<'a> = (&'a str, &'a str);
type PackageSet<'a> = Vec<Package<'a>>;

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
	let mut checked = HashMap::new();

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
	package_idx.iter().for_each(|(package, deps)| {
		check_packages(&package_idx, deps, &mut vec![*package], &mut checked);
	});

	let mut cyclic : Vec<(&Package, &PackageSet)> = checked.iter().filter(|(_k, v)| v.len() > 0).collect();

	if cyclic.is_empty() {
		Ok(())
	} else {
		let count = cyclic.len();
		cyclic.sort();
		for ((name, version), path) in cyclic {
			let mut view = path.iter().map(|(x, y)| format!("`{:}`(`{:}`)", x, y)).collect::<Vec<String>>();
			view.push(format!("`{:}`(`{:}`)", name, version));
			if opt.github {
				println!("- [ ] `{:}` (`{:}`): Cycle through {:}", name, version, view.join(" -> "));
			} else {
				println!("{:} ({:}): Cycle through {:}", name, version, view.join(" -> "));
			}
		}

		Err(format!("{} cyclic dependencies found", count))
	}
}

fn check_packages<'a>(
	idx: &HashMap<Package<'a>, PackageSet<'a>>,
	deps: &PackageSet<'a>,
	mut cur_path: &mut PackageSet<'a>,
	mut found: &mut HashMap<Package<'a>, PackageSet<'a>>,
) {
	deps.iter().for_each( |cur| {
		if found.contains_key(cur) {
			// skip already known cycles
			return
		}
		if let Some(pos) = cur_path.iter().position(|x| x == cur) {
			found.insert(*cur, cur_path.split_at(pos).1.to_vec());
			return
		}

		cur_path.push(*cur);
		idx.get(cur).map(|inner| check_packages(idx, inner, &mut cur_path, &mut found));
		cur_path.pop();
		found.entry(*cur).or_insert(vec![]);
	});
}

fn read_and_parse_toml(cargo_path: &Path) -> io::Result<toml::Value> {
	use std::io::Read;
	let mut file = fs::File::open(cargo_path)?;
	let mut contents = String::new();
	file.read_to_string(&mut contents)?;
	Ok(toml::from_str(&contents)?)
}