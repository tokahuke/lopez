use include_dir::{Dir, DirEntry};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{env, fs, io};

const LOPEZ_BIN: &[u8] = include_bytes!("../../target/release/lopez");
const LOPEZ_LIB: Dir = include_dir::include_dir!("../std-lopez");

fn install() -> io::Result<()> {
    let lib_path: PathBuf = "/usr/share/lopez/lib".parse().expect("infallible");

    println!("Installing `lopez` to `/usr/local/bin`");

    fs::write("/usr/local/bin/lopez", LOPEZ_BIN)?;
    fs::set_permissions("/usr/local/bin/lopez", fs::Permissions::from_mode(0o711))?;

    println!("Installing `std-lopez` to `usr/share/lopez`");

    println!("Creating folder structure");

    for entry in LOPEZ_LIB.find("**/*").expect("valid pattern") {
        match entry {
            DirEntry::Dir(dir) => {
                println!("... creating folder {:?}", dir.path());
                let path = lib_path.join(dir.path());
                fs::create_dir_all(&path)?;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
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
                let path = lib_path.join(file.path());
                fs::write(&path, file.contents())?;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            }
            _ => {
                // Already done...
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
