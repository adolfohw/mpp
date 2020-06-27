use mpp::Assembly;

// TODO: add tests
// TODO: actually make this a cli
// TODO: add a gui maybe?

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let src = std::fs::read_to_string(&path).unwrap();
    match Assembly::from_path(&path) {
        Ok(assembly) => print!("{:?}", assembly),
        Err(err) => err.throw(&src, &path, None),
    }
}
