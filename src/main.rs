use std::error::Error;

use rf::Finder;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello, world!");
    let finder = Finder::new_with_bash_history()?;
    Ok(())
}
