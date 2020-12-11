use std::error::Error;

use rf::Finder;

fn main() -> Result<(), Box<dyn Error>> {
    let mut finder = Finder::new_with_bash_history()?;
    // println!("{:?}", finder);
    finder.update_query(String::from("npmins"));
    let matches = finder.get_matched_commands();
    for c in matches {
        println!("{:?}\r\n", c);
    }
    Ok(())
}
