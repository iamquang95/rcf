use std::error::Error;

use rcf::Finder;

fn main() -> Result<(), Box<dyn Error>> {
    let mut finder = Finder::new_with_bash_history()?;
    
    finder.render()?;
    Ok(())
}
