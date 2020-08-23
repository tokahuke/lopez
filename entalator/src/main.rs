use include_dir::{Dir, DirEntry};
use std::path::PathBuf;
use std::{fs, io, env};

const LOPEZ_BIN: &[u8] = include_bytes!("../../target/release/lopez");
const LOPEZ_LIB: Dir = include_dir::include_dir!("../std-lopez");

fn install() -> io::Result<()> {
    let lib_path: PathBuf = "/usr/share/lopez/lib".parse().expect("infallible");

    println!("Installing `lopez` to `/usr/local/bin");

    fs::write("/usr/local/bin/lopez", LOPEZ_BIN)?;

    println!("Installing `std-lopez` to `usr/share/lopez`");

    println!("Creating folder structure");

    for entry in LOPEZ_LIB.find("**/*").expect("valid pattern") {
        match entry {
            DirEntry::Dir(dir) => {
                println!("... creating folder {:?}", dir.path());
                fs::create_dir_all(lib_path.join(dir.path()))?;
            }
            _ => {
                // Wait for it...
            }
        }
    }

    println!("Writing files");

    for entry in LOPEZ_LIB.find("**/*.lcd").expect("valid pattern") {
        match entry {
            DirEntry::File(file) => {
                println!("... writing file {:?}", file.path());
                fs::write(lib_path.join(file.path()), file.contents())?;
            }
            _ => {
                // Wait for it...
            }
        }
    }

    println!("\nErfolgreich!");

    Ok(())
}

fn uninstall() -> io::Result<()> {
    println!("Removing static application data in `/usr/share/lopez`");

    fs::remove_dir_all("/usr/share/lopez")?;

    println!("Removing lopez binary at `/usr/local/bin/lopez`");

    fs::remove_file("/usr/local/bin/lopez")?;

    println!("\nErfolgreich!");

    Ok(())
}

fn main() -> io::Result<()> {
    if !env::args().any(|arg| arg == "--uninstall" || arg == "-u") {
        install()
    } else {
        uninstall()
    }
}
