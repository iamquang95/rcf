use std::error::Error;

use rf::Finder;

fn main() -> Result<(), Box<dyn Error>> {
    let finder = Finder::new_with_bash_history()?;
    println!("{:?}", finder);
    Ok(())
}
