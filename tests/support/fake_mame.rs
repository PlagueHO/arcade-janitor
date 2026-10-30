use std::{env, fs, io::Write};

fn main() {
    assert_eq!(env::args().skip(1).collect::<Vec<_>>(), ["-listxml"]);
    let mut calls = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(env::var_os("FAKE_MAME_CALLS").unwrap())
        .unwrap();
    writeln!(calls, "-listxml").unwrap();
    match env::var("FAKE_MAME_MODE").as_deref() {
        Ok("fail") => {
            eprintln!("deterministic MAME failure");
            std::process::exit(7);
        }
        Ok("invalid") => print!("<mame><machine"),
        Ok("empty") => print!("<mame/>"),
        Ok("encoding") => std::io::stdout().write_all(&[0xff]).unwrap(),
        _ => print!(
            "{}",
            fs::read_to_string(env::var_os("FAKE_MAME_XML").unwrap()).unwrap()
        ),
    }
}
