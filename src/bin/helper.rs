use grub_studio::admin;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match admin::main_admin(&args) {
        Ok(result) => print!("{result}"),
        Err(e) => { eprintln!("GRUB Studio: {e}"); std::process::exit(1); }
    }
}
