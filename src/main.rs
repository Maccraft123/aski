pub mod lib;
use crate::lib::Picker;
use std::io;

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("1st argument has to be prompt for selection");
        eprintln!(r"All stdin input is options, separated by a \n character");
        return Ok(());
    }

    let mut opts = Vec::new();
    loop {
        let mut buf = String::new();
        if let Ok(n) = io::stdin().read_line(&mut buf) {
            // Ok(0) is EOF
            if n == 0 {
                break;
            }
            opts.push(buf.trim().to_string());
        }
    }

    let mut picker = Picker::new(args[1].clone());
    picker.add_options(opts).unwrap();

    let response = picker.wait_choice()?;

    println!("{}", response);
    Ok(())
}
